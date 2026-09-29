use super::*;
use crate::{Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchRequestV1};
use MixedConditionalIndexDomainV26 as Domain;

fn geometry() -> AqlDispatchGeometryV1 {
    AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap()
}
fn slices() -> [Slice; 3] {
    std::array::from_fn(|i| Slice {
        generated_field: i as u16,
        pointer_offset: i * 16,
        length_offset: i * 16 + 8,
        buffer_index: Some(i),
        buffer_byte_offset: 0,
        length: 3,
        element_bytes: 4,
        alignment: 4,
    })
}
fn access(slice: u16, writing: bool, identity: u8) -> MixedConditionalAccessV26 {
    MixedConditionalAccessV26 {
        slice,
        writing,
        occurrence_identity: [identity; 32],
        access_domain: Domain::LogicalExtent { slice },
        address_domain: Domain::InvocationAxis { axis: 0 },
        invocation_axis: Some(0),
    }
}
fn accesses() -> [MixedConditionalAccessV26; 4] {
    [
        access(0, true, 1),
        access(0, false, 2),
        access(1, true, 3),
        access(2, false, 4),
    ]
}
fn bytes(slices: &[Slice]) -> Vec<u8> {
    let mut bytes = vec![0; slices.len() * 16];
    for slice in slices {
        bytes[slice.length_offset..slice.length_offset + 8]
            .copy_from_slice(&slice.length.to_le_bytes());
    }
    bytes
}
fn payload(
    slices: &[Slice],
    accesses: &[MixedConditionalAccessV26],
) -> Result<MixedConditionalDispatchPremisesV26> {
    MixedConditionalDispatchPremisesV26::new(
        [1; 32],
        [2; 32],
        [3; 32],
        &bytes(slices),
        geometry(),
        3,
        [64, 1, 1],
        64,
        slices,
        accesses,
    )
}
fn request(slices: &[Slice]) -> Gfx942KfdDispatchRequestV1 {
    Gfx942KfdDispatchRequestV1::new(
        vec![0; 128],
        64,
        bytes(slices),
        8,
        (0..slices.len())
            .map(|_| Gfx942KfdDispatchBufferV1::new(vec![0; 12]).unwrap())
            .collect(),
        slices
            .iter()
            .filter_map(|slice| {
                slice.buffer_index.map(|i| {
                    Gfx942KfdDispatchPointerFixupV1::new(
                        slice.pointer_offset,
                        i,
                        slice.buffer_byte_offset,
                        u64::from(slice.alignment),
                    )
                })
            })
            .collect(),
        geometry(),
        0,
        0,
        1000,
    )
    .unwrap()
}
fn facts(base: u64, logical: usize, id: u64) -> SharedGttMappedResourceFactsV1 {
    SharedGttMappedResourceFactsV1::conditional_test_facts_v1(base, logical, 4096, id, 1)
}

#[test]
fn mixed_multiple_writes_and_same_argument_reads_reach_actual_request() {
    let slices = slices();
    let payload = payload(&slices, &accesses()).unwrap();
    let identity = *payload.identity();
    let request = request(&slices)
        .with_mixed_conditional_premises_v26(payload)
        .unwrap();
    assert!(request.conditional_premises_v1().is_none());
    let retained = request.mixed_conditional_premises_v26().unwrap();
    assert_eq!(retained.identity(), &identity);
    assert!(
        retained
            .check_live(&[
                facts(0x1000, 12, 1),
                facts(0x2000, 12, 2),
                facts(0x3000, 12, 3)
            ])
            .is_ok()
    );
}

#[test]
fn mixed_full_span_writable_aliases_refuse_but_read_only_aliases_do_not() {
    let slices = slices();
    let mixed = payload(&slices, &accesses()).unwrap();
    for bases in [
        [0x1000, 0x1004, 0x3000],
        [0x1000, 0x2000, 0x1004],
        [0x1000, 0x2000, 0x2008],
    ] {
        assert_eq!(
            mixed.check_live(&std::array::from_fn::<_, 3, _>(|i| facts(
                bases[i], 12, i as u64
            ))),
            Err(Error::Alias)
        );
    }
    assert!(
        mixed
            .check_live(&[
                facts(0x1000, 12, 1),
                facts(0x100c, 12, 2),
                facts(0x1018, 12, 3)
            ])
            .is_ok()
    );
    let readonly = payload(
        &slices,
        &[
            access(0, false, 1),
            access(1, false, 2),
            access(2, false, 3),
        ],
    )
    .unwrap();
    assert!(
        readonly
            .check_live(&[
                facts(0x1000, 12, 1),
                facts(0x1000, 12, 1),
                facts(0x1000, 12, 1)
            ])
            .is_ok()
    );
}

#[test]
fn mixed_formation_domain_is_not_replaced_by_dereference_guard() {
    let slices = slices();
    let mut rows = accesses();
    let live = [
        facts(u64::MAX - 15, 12, 1),
        facts(0x2000, 12, 2),
        facts(0x3000, 12, 3),
    ];
    assert_eq!(
        payload(&slices, &rows).unwrap().check_live(&live),
        Err(Error::Arithmetic)
    );
    rows[0].address_domain = Domain::LogicalExtent { slice: 0 };
    rows[1].address_domain = Domain::LogicalExtent { slice: 0 };
    assert!(payload(&slices, &rows).unwrap().check_live(&live).is_ok());
    rows[2].address_domain = Domain::UnsignedWidth { bits: 64 };
    assert_eq!(
        payload(&slices, &rows).unwrap().check_live(&live),
        Err(Error::Arithmetic)
    );
}

#[test]
fn mixed_exact_launch_projection_width_and_complete_roster_are_required() {
    let slices = slices();
    let rows = accesses();
    for (rank, grid, width) in [
        (0, [64, 1, 1], 64),
        (4, [64, 1, 1], 64),
        (1, [63, 1, 1], 64),
        (1, [64, 1, 1], 16),
    ] {
        assert!(
            MixedConditionalDispatchPremisesV26::new(
                [1; 32],
                [2; 32],
                [3; 32],
                &bytes(&slices),
                geometry(),
                rank,
                grid,
                width,
                &slices,
                &rows
            )
            .is_err()
        );
    }
    let mut bad = rows;
    bad[1].invocation_axis = None;
    assert!(payload(&slices, &bad).is_err());
    bad = rows;
    bad[2].occurrence_identity = bad[0].occurrence_identity;
    assert!(payload(&slices, &bad).is_err());
    assert!(payload(&slices, &rows[..3]).is_err());
    bad = rows;
    bad[3].access_domain = Domain::InvocationAxis { axis: 0 };
    assert!(matches!(payload(&slices, &bad), Err(Error::InputExtent)));
    assert!(matches!(
        payload(&slices, &vec![rows[0]; MAX_ACCESSES + 1]),
        Err(Error::ResourceLimit)
    ));
}

#[test]
fn mixed_request_substitution_and_replacement_are_closed() {
    let slices = slices();
    let p = payload(&slices, &accesses()).unwrap();
    let mut wrong = slices;
    wrong[0].length = 2;
    assert!(
        request(&wrong)
            .with_mixed_conditional_premises_v26(p)
            .is_err()
    );
    let first = payload(&slices, &accesses()).unwrap();
    let second = payload(&slices, &accesses()).unwrap();
    assert!(
        request(&slices)
            .with_mixed_conditional_premises_v26(first)
            .unwrap()
            .with_mixed_conditional_premises_v26(second)
            .is_err()
    );
    let short = [
        facts(0x1000, 11, 1),
        facts(0x2000, 12, 2),
        facts(0x3000, 12, 3),
    ];
    assert_eq!(
        payload(&slices, &accesses()).unwrap().check_live(&short),
        Err(Error::LogicalSpan)
    );
}

#[test]
fn mixed_identity_binds_occurrence_and_formation_domains_with_unchanged_counts() {
    let slices = slices();
    let rows = accesses();
    let before = payload(&slices, &rows).unwrap();
    let mut changed = rows;
    changed[0].occurrence_identity = [9; 32];
    assert_ne!(
        before.identity(),
        payload(&slices, &changed).unwrap().identity()
    );
    changed = rows;
    changed[0].address_domain = Domain::LogicalExtent { slice: 0 };
    assert_ne!(
        before.identity(),
        payload(&slices, &changed).unwrap().identity()
    );
    changed = rows;
    changed.swap(0, 1);
    assert_ne!(
        before.identity(),
        payload(&slices, &changed).unwrap().identity()
    );
}

#[test]
fn mixed_width_empty_extent_and_offset_overflow_boundaries_are_exact() {
    let mut slice = slices()[0];
    slice.length = u64::from(u32::MAX);
    let row = access(0, false, 1);
    let construct = |slice: Slice| {
        MixedConditionalDispatchPremisesV26::new(
            [1; 32],
            [2; 32],
            [3; 32],
            &bytes(&[slice]),
            geometry(),
            3,
            [64, 1, 1],
            32,
            &[slice],
            &[row],
        )
    };
    assert!(construct(slice).is_ok());
    slice.length += 1;
    assert!(matches!(construct(slice), Err(Error::Arithmetic)));

    slice.length = 0;
    slice.buffer_index = None;
    let mut empty = row;
    empty.address_domain = Domain::LogicalExtent { slice: 0 };
    let p = payload(&[slice], &[empty]).unwrap();
    assert!(
        p.check_request(&bytes(&[slice]), &[], [].into_iter(), geometry())
            .is_ok()
    );
    assert!(p.check_live(&[]).is_ok());

    slice = slices()[0];
    slice.buffer_byte_offset = usize::MAX - 7;
    let p = payload(&[slice], &[row]).unwrap();
    let fixup = Gfx942KfdDispatchPointerFixupV1::new(0, 0, slice.buffer_byte_offset, 4);
    assert_eq!(
        p.check_request(
            &bytes(&[slice]),
            &[fixup],
            std::iter::once(usize::MAX),
            geometry()
        ),
        Err(Error::Arithmetic)
    );
}
