use super::*;
use crate::{Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchRequestV1};

#[test]
fn bounded_copies_reject_excess_capacity_before_retaining_rows() {
    let values = [slice(0, 3, Some(0)), slice(1, 3, Some(1))];
    let reserved = Vec::with_capacity(values.len());
    let pointer = reserved.as_ptr();
    let exact = copy_into_exact_storage(reserved, &values).unwrap();
    assert_eq!(exact.as_ptr(), pointer);
    assert_eq!(exact, values);
    assert_eq!(exact.capacity(), values.len());
    assert_eq!(bounded_copy(&values).unwrap(), values);
    let empty = bounded_copy::<ConditionalDispatchReadV1>(&[]).unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.capacity(), 0);
    for capacity in [values.len() - 1, values.len() + 1, MAX_SLICES] {
        assert!(matches!(
            copy_into_exact_storage(Vec::with_capacity(capacity), &values),
            Err(ConditionalDispatchErrorV1::Allocation)
        ));
    }
    assert!(matches!(
        copy_into_exact_storage(Vec::<ConditionalDispatchReadV1>::with_capacity(1), &[]),
        Err(ConditionalDispatchErrorV1::Allocation)
    ));
    assert!(matches!(
        copy_into_exact_storage(values.to_vec(), &values),
        Err(ConditionalDispatchErrorV1::Allocation)
    ));
}
fn geometry(g: u32) -> AqlDispatchGeometryV1 {
    AqlDispatchGeometryV1::new([g, 1, 1], [g.min(64), 1, 1]).unwrap()
}
fn slice(field: u16, length: u64, buffer: Option<usize>) -> ConditionalDispatchSliceV1 {
    ConditionalDispatchSliceV1 {
        generated_field: field,
        pointer_offset: usize::from(field) * 16,
        length_offset: usize::from(field) * 16 + 8,
        buffer_index: buffer,
        buffer_byte_offset: 0,
        length,
        element_bytes: 4,
        alignment: 4,
    }
}
fn kernarg(slices: &[ConditionalDispatchSliceV1]) -> Vec<u8> {
    let mut bytes = vec![0; slices.iter().map(|s| s.length_offset + 8).max().unwrap()];
    for s in slices {
        bytes[s.length_offset..s.length_offset + 8].copy_from_slice(&s.length.to_le_bytes());
    }
    bytes
}
fn premises(
    slices: &[ConditionalDispatchSliceV1],
    reads: &[ConditionalDispatchReadV1],
    g: u32,
) -> Result<ConditionalDispatchPremisesV1> {
    ConditionalDispatchPremisesV1::new(
        [1; 32],
        [2; 32],
        [3; 32],
        &kernarg(slices),
        geometry(g),
        slices,
        0,
        ConditionalDispatchDomainV1::GuardedOutput,
        reads,
    )
}
fn read(
    slice: u16,
    access_domain: ConditionalDispatchDomainV1,
    address_domain: ConditionalDispatchDomainV1,
) -> ConditionalDispatchReadV1 {
    ConditionalDispatchReadV1 {
        slice,
        access_domain,
        address_domain,
    }
}
fn guarded(slice: u16) -> ConditionalDispatchReadV1 {
    read(
        slice,
        ConditionalDispatchDomainV1::GuardedOutput,
        ConditionalDispatchDomainV1::GuardedOutput,
    )
}
fn facts(base: u64, logical: usize, id: u64, generation: u64) -> SharedGttMappedResourceFactsV1 {
    SharedGttMappedResourceFactsV1::conditional_test_facts_v1(base, logical, 4096, id, generation)
}
fn request(slices: &[ConditionalDispatchSliceV1], sizes: &[usize]) -> Gfx942KfdDispatchRequestV1 {
    let fixups = slices
        .iter()
        .filter_map(|s| {
            s.buffer_index.map(|i| {
                Gfx942KfdDispatchPointerFixupV1::new(
                    s.pointer_offset,
                    i,
                    s.buffer_byte_offset,
                    u64::from(s.alignment),
                )
            })
        })
        .collect();
    Gfx942KfdDispatchRequestV1::new(
        vec![0; 128],
        64,
        kernarg(slices),
        8,
        sizes
            .iter()
            .map(|n| Gfx942KfdDispatchBufferV1::new(vec![0; *n]).unwrap())
            .collect(),
        fixups,
        geometry(64),
        0,
        0,
        1000,
    )
    .unwrap()
}

#[test]
fn actual_request_transition_accepts_unpadded_guarded_domains() {
    let s = [slice(0, 3, Some(0)), slice(1, 3, Some(1))];
    let p = premises(&s, &[guarded(1)], 64).unwrap();
    assert!(
        p.check_live(&[facts(0x1000, 12, 1, 1), facts(0x2000, 12, 2, 1)])
            .is_ok()
    );
    assert!(
        request(&s, &[12, 12])
            .with_conditional_premises_v1(p)
            .is_ok()
    );
}

#[test]
fn global_address_formation_does_not_require_global_readable_padding() {
    let s = [slice(0, 3, Some(0)), slice(1, 3, Some(1))];
    let r = read(
        1,
        ConditionalDispatchDomainV1::GuardedOutput,
        ConditionalDispatchDomainV1::GlobalLaunch,
    );
    let p = premises(&s, &[r], 64).unwrap();
    assert!(
        p.check_live(&[facts(0x1000, 12, 1, 1), facts(0x2000, 12, 2, 1)])
            .is_ok()
    );
    let r = read(
        1,
        ConditionalDispatchDomainV1::GlobalLaunch,
        ConditionalDispatchDomainV1::GlobalLaunch,
    );
    assert!(matches!(
        premises(&s, &[r], 64),
        Err(ConditionalDispatchErrorV1::InputExtent)
    ));
    let s = [slice(0, 3, Some(0)), slice(1, 64, Some(1))];
    assert!(
        premises(&s, &[r], 64)
            .unwrap()
            .check_live(&[facts(0x1000, 12, 1, 1), facts(0x2000, 256, 2, 1),])
            .is_ok()
    );
}

#[test]
fn empty_guarded_spans_still_check_formed_zero_address() {
    let s = [slice(0, 0, None), slice(1, 0, None)];
    let p = premises(&s, &[guarded(1)], 64).unwrap();
    assert!(p.check_live(&[]).is_ok());
    assert!(request(&s, &[]).with_conditional_premises_v1(p).is_ok());
    assert_eq!(
        check_address(&s[0], 3, ConditionalDispatchDomainV1::GuardedOutput, 0, 64),
        Err(ConditionalDispatchErrorV1::Alignment)
    );
    assert!(check_address(&s[0], 3, ConditionalDispatchDomainV1::GuardedOutput, 0, 0).is_ok());
    assert!(AqlDispatchGeometryV1::new([0, 1, 1], [64, 1, 1]).is_err());
}

#[test]
fn rejects_undercoverage_and_non_d1_geometry() {
    let s = [slice(0, 65, Some(0))];
    assert!(matches!(
        premises(&s, &[], 64),
        Err(ConditionalDispatchErrorV1::OutputExtent)
    ));
    let s = [slice(0, 1, Some(0))];
    let wrong = AqlDispatchGeometryV1::new([64, 2, 1], [64, 1, 1]).unwrap();
    assert!(matches!(
        ConditionalDispatchPremisesV1::new(
            [1; 32],
            [2; 32],
            [3; 32],
            &kernarg(&s),
            wrong,
            &s,
            0,
            ConditionalDispatchDomainV1::GuardedOutput,
            &[]
        ),
        Err(ConditionalDispatchErrorV1::Geometry)
    ));
}

#[test]
fn rounded_mapped_pages_do_not_prove_logical_span() {
    let s = [slice(0, 3, Some(0))];
    let p = premises(&s, &[], 64).unwrap();
    assert_eq!(
        p.check_live(&[facts(0x1000, 8, 1, 1)]),
        Err(ConditionalDispatchErrorV1::LogicalSpan)
    );
    assert!(request(&s, &[8]).with_conditional_premises_v1(p).is_err());
}

#[test]
fn separation_covers_entire_logical_input_not_only_read_prefix() {
    let s = [slice(0, 1, Some(0)), slice(1, 8, Some(1))];
    let p = premises(&s, &[guarded(1)], 64).unwrap();
    assert_eq!(
        p.check_live(&[facts(0x1010, 4, 1, 1), facts(0x1000, 32, 2, 1)]),
        Err(ConditionalDispatchErrorV1::Alias)
    );
    assert!(
        p.check_live(&[facts(0x1020, 4, 1, 1), facts(0x1000, 32, 2, 1)])
            .is_ok()
    );
}

#[test]
fn read_only_inputs_may_alias_each_other() {
    let s = [
        slice(0, 3, Some(0)),
        slice(1, 3, Some(1)),
        slice(2, 3, Some(1)),
    ];
    let p = premises(&s, &[guarded(1), guarded(2)], 64).unwrap();
    assert!(
        p.check_live(&[facts(0x1000, 12, 1, 1), facts(0x2000, 12, 2, 1)])
            .is_ok()
    );
    assert!(
        request(&s, &[12, 12])
            .with_conditional_premises_v1(p)
            .is_ok()
    );
}

#[test]
fn rejects_unaligned_and_overflowing_actual_gpu_addresses() {
    let s = [slice(0, 1, Some(0)), slice(1, 1, Some(1))];
    let r = read(
        1,
        ConditionalDispatchDomainV1::GuardedOutput,
        ConditionalDispatchDomainV1::GlobalLaunch,
    );
    let p = premises(&s, &[r], 64).unwrap();
    assert_eq!(
        p.check_live(&[facts(0x1001, 4, 1, 1), facts(0x2000, 4, 2, 1)]),
        Err(ConditionalDispatchErrorV1::Alignment)
    );
    assert_eq!(
        p.check_live(&[facts(0x1000, 4, 1, 1), facts(u64::MAX - 7, 4, 2, 1)]),
        Err(ConditionalDispatchErrorV1::Arithmetic)
    );
    assert_eq!(
        check_address(
            &s[1],
            0,
            ConditionalDispatchDomainV1::GlobalLaunch,
            0,
            u64::MAX
        ),
        Err(ConditionalDispatchErrorV1::Arithmetic)
    );
    let huge = [slice(0, 0, None), slice(1, u64::MAX, Some(0))];
    assert!(matches!(
        premises(&huge, &[guarded(1)], 64),
        Err(ConditionalDispatchErrorV1::Arithmetic)
    ));
}

#[test]
fn request_rejects_wrong_kernarg_fixup_geometry_or_duplicate_attachment() {
    let s = [slice(0, 3, Some(0)), slice(1, 3, Some(1))];
    let p = premises(&s, &[guarded(1)], 64).unwrap();
    let f = [
        Gfx942KfdDispatchPointerFixupV1::new(0, 1, 0, 4),
        Gfx942KfdDispatchPointerFixupV1::new(16, 0, 0, 4),
    ];
    assert_eq!(
        p.check_request(&kernarg(&s), &f, [12, 12].into_iter(), geometry(64)),
        Err(ConditionalDispatchErrorV1::Binding)
    );
    let f = [
        Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4),
        Gfx942KfdDispatchPointerFixupV1::new(16, 1, 0, 4),
    ];
    let mut bytes = kernarg(&s);
    bytes[8] = 2;
    assert_eq!(
        p.check_request(&bytes, &f, [12, 12].into_iter(), geometry(64)),
        Err(ConditionalDispatchErrorV1::Binding)
    );
    assert_eq!(
        p.check_request(&kernarg(&s), &f, [12, 12].into_iter(), geometry(32)),
        Err(ConditionalDispatchErrorV1::Geometry)
    );
    let req = request(&s, &[12, 12])
        .with_conditional_premises_v1(p)
        .unwrap();
    assert!(
        req.with_conditional_premises_v1(premises(&s, &[guarded(1)], 64).unwrap())
            .is_err()
    );
}

#[test]
fn rejects_unknown_unused_or_excessive_rows_and_address_bearing_input() {
    let s = [slice(0, 1, Some(0)), slice(1, 1, Some(1))];
    assert!(matches!(
        premises(&s, &[], 64),
        Err(ConditionalDispatchErrorV1::Binding)
    ));
    let mut bytes = kernarg(&s);
    bytes[0] = 1;
    assert!(matches!(
        ConditionalDispatchPremisesV1::new(
            [1; 32],
            [2; 32],
            [3; 32],
            &bytes,
            geometry(64),
            &s,
            0,
            ConditionalDispatchDomainV1::GuardedOutput,
            &[guarded(1)]
        ),
        Err(ConditionalDispatchErrorV1::Binding)
    ));
    let reads = vec![guarded(1); MAX_READS + 1];
    assert!(matches!(
        premises(&s, &reads, 64),
        Err(ConditionalDispatchErrorV1::ResourceLimit)
    ));
    assert!(matches!(
        premises(&s, &[guarded(2)], 64),
        Err(ConditionalDispatchErrorV1::Binding)
    ));
}

#[test]
fn generation_and_mapping_continuity_are_not_address_equality() {
    let original = facts(0x1000, 12, 1, 1);
    assert!(require_same_mapping_v1(&original, &facts(0x1000, 12, 1, 1)).is_ok());
    for changed in [
        facts(0x1000, 12, 1, 2),
        facts(0x1000, 12, 2, 1),
        facts(0x1000, 16, 1, 1),
        facts(0x2000, 12, 1, 1),
        facts(0x1000, 12, 1, 1).with_conditional_test_publication_v1(2),
        SharedGttMappedResourceFactsV1::conditional_test_facts_v1(0x1000, 12, 8192, 1, 1),
    ] {
        assert_eq!(
            require_same_mapping_v1(&original, &changed),
            Err(ConditionalDispatchErrorV1::StaleMapping)
        );
    }
}

#[test]
fn full_span_exact_and_one_short_extents_are_checked_before_dispatch() {
    let s = [slice(0, 3, Some(0))];
    let p = premises(&s, &[], 64).unwrap();
    assert!(
        p.check_live(&[SharedGttMappedResourceFactsV1::conditional_test_facts_v1(
            0x1000, 12, 12, 1, 1,
        )])
        .is_ok()
    );
    for (logical, mapped) in [(11, 12), (12, 11)] {
        assert_eq!(
            p.check_live(&[SharedGttMappedResourceFactsV1::conditional_test_facts_v1(
                0x1000, logical, mapped, 1, 1,
            )]),
            Err(ConditionalDispatchErrorV1::LogicalSpan)
        );
    }
}

#[test]
fn global_tail_address_has_an_exact_checked_arithmetic_boundary() {
    let s = [slice(0, 3, Some(0)), slice(1, 3, Some(1))];
    let p = premises(
        &s,
        &[read(
            1,
            ConditionalDispatchDomainV1::GuardedOutput,
            ConditionalDispatchDomainV1::GlobalLaunch,
        )],
        64,
    )
    .unwrap();
    let last_valid_base = u64::MAX - 255;
    assert!(
        p.check_live(&[facts(0x1000, 12, 1, 1), facts(last_valid_base, 12, 2, 1)])
            .is_ok()
    );
    assert_eq!(
        p.check_live(&[
            facts(0x1000, 12, 1, 1),
            facts(last_valid_base + 4, 12, 2, 1)
        ]),
        Err(ConditionalDispatchErrorV1::Arithmetic)
    );
}

#[test]
fn adjacent_subspans_may_share_a_mapping_but_output_overlap_is_rejected() {
    let mut s = [slice(0, 3, Some(0)), slice(1, 3, Some(0))];
    s[1].buffer_byte_offset = 12;
    let p = premises(&s, &[guarded(1)], 64).unwrap();
    assert!(p.check_live(&[facts(0x1000, 24, 1, 1)]).is_ok());
    assert!(request(&s, &[24]).with_conditional_premises_v1(p).is_ok());
    s[1].buffer_byte_offset = 8;
    assert_eq!(
        premises(&s, &[guarded(1)], 64)
            .unwrap()
            .check_live(&[facts(0x1000, 24, 1, 1)]),
        Err(ConditionalDispatchErrorV1::Alias)
    );
}

#[test]
fn identity_binds_contract_packing_kernel_geometry_domains_and_occurrences() {
    let s = [slice(0, 3, Some(0)), slice(1, 3, Some(1))];
    let make = |c, p, k, g, reads: &[ConditionalDispatchReadV1]| {
        ConditionalDispatchPremisesV1::new(
            c,
            p,
            k,
            &kernarg(&s),
            geometry(g),
            &s,
            0,
            ConditionalDispatchDomainV1::GuardedOutput,
            reads,
        )
        .unwrap()
    };
    let base = make([1; 32], [2; 32], [3; 32], 64, &[guarded(1)]);
    for changed in [
        make([4; 32], [2; 32], [3; 32], 64, &[guarded(1)]),
        make([1; 32], [4; 32], [3; 32], 64, &[guarded(1)]),
        make([1; 32], [2; 32], [4; 32], 64, &[guarded(1)]),
        make([1; 32], [2; 32], [3; 32], 32, &[guarded(1)]),
        make([1; 32], [2; 32], [3; 32], 64, &[guarded(1), guarded(1)]),
        make(
            [1; 32],
            [2; 32],
            [3; 32],
            64,
            &[read(
                1,
                ConditionalDispatchDomainV1::GuardedOutput,
                ConditionalDispatchDomainV1::GlobalLaunch,
            )],
        ),
    ] {
        assert_ne!(base.identity(), changed.identity());
    }
}
