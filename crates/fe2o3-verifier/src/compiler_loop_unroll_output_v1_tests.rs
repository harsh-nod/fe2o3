use super::super::{joins as shared, tests as old};
use super::*;
use fe2o3_compiler_ffi::{
    INERT_LOOP_UNROLL_ENCODE_STORAGE_V1, InertLoopUnrollRootRefV1 as Root,
    encode_inert_loop_unroll_output_into_v1, inert_loop_unroll_output_len_v1,
    read_inert_loop_unroll_output_v1, read_inert_refined_forwarding_output_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[allow(
    dead_code,
    reason = "reuse source construction only, not fixture signing helpers"
)]
#[path = "compiler_refined_forwarding_output_fixture_v1_tests.rs"]
mod fixture_support;

// Opaque framing component, not a decoded history or a signed-source positive.
// Retain the exact old F component fields inside a distinct nine-field U frame.
fn fields() -> [Vec<u8>; 14] {
    let mut out = old::fields();
    let mut fields: [Vec<u8>; 9] = std::array::from_fn(|_| Vec::new());
    fields[0] = out[0].clone();
    fields[1] = vec![0x55];
    for family in 0..6 {
        fields[2 + family] = vec![0, 0, 0, 0, family as u8, 1, 0, 0];
    }
    fields[8] = vec![0; 104];
    fields[8][56] = 8;
    let total = 104 + fields.iter().map(Vec::len).sum::<usize>();
    let mut history = vec![0; 104];
    history[..8].copy_from_slice(b"F2LUH1\0\0");
    history[8..12].copy_from_slice(&[1, 0, 1, 0]);
    history[12..16].copy_from_slice(&104u32.to_le_bytes());
    history[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    history[24] = 9;
    for (i, field) in fields.iter().enumerate() {
        history[32 + 8 * i..40 + 8 * i].copy_from_slice(&(field.len() as u64).to_le_bytes());
        history.extend_from_slice(field);
    }
    out[0] = history;
    out
}

fn framed(fields: &[Vec<u8>; 14]) -> Vec<u8> {
    let borrowed = fields.each_ref().map(Vec::as_slice);
    let mut out = vec![0; inert_loop_unroll_output_len_v1::<Resource>(&borrowed).unwrap()];
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    let floor = fields.iter().map(Vec::capacity).sum::<usize>()
        + size_of::<[Vec<u8>; 14]>()
        + size_of::<Vec<u8>>()
        + out.capacity();
    budget
        .reserve_storage(floor + INERT_LOOP_UNROLL_ENCODE_STORAGE_V1)
        .unwrap();
    encode_inert_loop_unroll_output_into_v1(
        borrowed,
        fe2o3_compiler_ffi::InertLoopUnrollRouteV1::Direct,
        &mut out,
        MAX_STORAGE,
        |n| budget.charge_work(n),
    )
    .unwrap();
    out
}

fn read<'a>(wire: &'a [u8], budget: &mut Budget<'_>) -> Frame<'a> {
    budget
        .reserve_storage(READ_STORAGE + size_of::<Frame<'_>>())
        .unwrap();
    let frame =
        read_inert_loop_unroll_output_v1(wire, MAX_STORAGE, |n| budget.charge_work(n)).unwrap();
    budget.release_storage(READ_STORAGE).unwrap();
    // The returned view is separately retained across all later paid operations.
    frame
}

fn association_run(
    fields: &[Vec<u8>; 14],
    w: usize,
    p: usize,
) -> (R<()>, usize, usize, Option<usize>) {
    let wire = framed(fields);
    let sibling = [0x53u8; 37];
    let mut work = Work::new(w);
    let mut budget = Budget::new(&mut work, p);
    budget.charge_work(11).unwrap();
    budget
        .reserve_storage(
            size_of::<Vec<u8>>()
                + wire.capacity()
                + size_of::<[u8; 37]>()
                + size_of::<[Vec<u8>; 14]>()
                + fields.iter().map(Vec::capacity).sum::<usize>(),
        )
        .unwrap();
    let frame = read(&wire, &mut budget);
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = joins::association(&frame, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x53; 37]);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn final_u_association_components_join_all_five_domains_and_exact_resources() {
    let fields = fields();
    let (result, work, peak, failed) = association_run(&fields, usize::MAX, MAX_STORAGE);
    result.unwrap();
    assert_eq!(failed, None);
    let (result, w, p, failed) = association_run(&fields, work, peak);
    result.unwrap();
    assert_eq!((w, p, failed), (work, peak, None));
    let (result, w, p, failed) = association_run(&fields, work - 1, peak);
    match result {
        Err(E::Join(super::super::E::Resource(Resource::Work(error)))) => {
            assert_eq!((error.actual(), error.limit()), (work, work - 1));
        }
        other => panic!("exact last identity comparison: {other:?}"),
    }
    assert_eq!((w, p, failed), (work - 80, peak, None));
}

#[test]
fn final_u_association_refuses_each_foreign_original_stage_and_signature_payload() {
    let original = fields();
    for (index, message) in [
        (3, "V4 SemanticMir identity/length"),
        (10, "V4 OriginalMiddleEnd identity/length"),
        (4, "V4 OriginalNative identity/length"),
        (11, "V4 OriginalCorrespondence identity/length"),
        (7, "V4 OriginalFormalMemory identity/length"),
        (12, "V4 exact signed Verus bytes"),
    ] {
        let mut bad = original.clone();
        bad[index].push(0xa5);
        let result = association_run(&bad, usize::MAX, MAX_STORAGE);
        assert!(
            matches!(result.0, Err(E::Join(super::super::E::Mismatch(actual))) if actual == message)
        );
        assert_eq!(result.3, None);
    }
}

#[test]
fn final_u_framing_identity_and_typed_error_families_remain_distinct() {
    use std::error::Error;
    let fields = fields();
    let wire = framed(&fields);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget
        .reserve_storage(size_of::<Vec<u8>>() + wire.capacity())
        .unwrap();
    let frame = read(&wire, &mut budget);
    assert!(!frame.grants_authority());
    budget
        .reserve_storage(HASH_STORAGE + size_of::<Identity>() * 2)
        .unwrap();
    let a = inert_loop_unroll_output_identity_v1(&frame, MAX_STORAGE, |n| budget.charge_work(n))
        .unwrap();
    let b = inert_loop_unroll_history_identity_v1(frame.field(Field::History), MAX_STORAGE, |n| {
        budget.charge_work(n)
    })
    .unwrap();
    assert_ne!(a.sha256, b.sha256);
    assert_eq!(a.byte_len, wire.len() as u64);
    assert_eq!(b.byte_len, fields[0].len() as u64);
    assert!(matches!(
        read_inert_refined_forwarding_output_v1(&wire, MAX_STORAGE, |n| budget.charge_work(n)),
        Err(fe2o3_compiler_ffi::InertRefinedForwardingOutputErrorV1::Header)
    ));
    let error = E::Framing(FrameError::Charge(Resource::Accounting));
    assert!(error.source().unwrap().downcast_ref::<Resource>().is_some());
    assert!(E::Framing(FrameError::Header).source().is_none());
    assert!(
        E::History(LoopUnrollHistoryErrorV1::Mismatch("typed history"))
            .source()
            .is_some()
    );
    assert!(
        E::Join(super::super::E::Mismatch("typed source"))
            .source()
            .is_some()
    );
}

#[test]
fn final_u_input_floor_counts_shared_history_once_and_separate_backing_twice() {
    let fields = fields();
    let wire = framed(&fields);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget
        .reserve_storage(size_of::<Vec<u8>>() + wire.capacity())
        .unwrap();
    let frame = read(&wire, &mut budget);
    let shared_bytes = frame.field(Field::History);
    let owned = shared_bytes.to_vec();
    assert_eq!(shared_bytes, owned);
    assert!(!std::ptr::eq(shared_bytes, owned.as_slice()));
    // Arithmetic component only: no fabricated decoded-history/source owner.
    let expected = 71 + wire.len() + size_of::<Frame<'_>>() + 113 + 127;
    assert_eq!(
        backing_floor(71, &frame, shared_bytes, 113, 127).unwrap(),
        expected
    );
    assert_eq!(
        backing_floor(71, &frame, &owned, 113, 127).unwrap(),
        expected + owned.len()
    );
    assert!(matches!(
        backing_floor(usize::MAX, &frame, shared_bytes, 113, 127),
        Err(E::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn final_u_header_components_preflight_exact_typed_extents_before_work() {
    use fe2o3_compiler_lineage::MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3 as ROOTS;
    let owner_header = size_of::<Checked<'_, '_, '_>>() + size_of::<Inputs<'_>>() + READ_STORAGE;
    let root_header = READ_STORAGE + size_of::<Root<'_>>() + size_of::<[[bool; ROOTS]; 3]>();
    assert_eq!(header_storage().unwrap(), owner_header);
    assert_eq!(joins::root_working().unwrap(), root_header);
    for extent in [owner_header, root_header] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 53 + extent - 1);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(53).unwrap();
        let error = scoped(&mut budget, |b| {
            b.reserve_storage(extent).map_err(E::Resource)
        })
        .unwrap_err();
        match error {
            E::Resource(Resource::Storage(error)) => {
                assert_eq!(
                    (error.actual(), error.limit()),
                    (53 + extent, 53 + extent - 1)
                );
            }
            other => panic!("exact component header refusal: {other:?}"),
        }
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (7, 53, 53, Some(53 + extent))
        );
    }
}

#[test]
fn final_u_nested_scope_preserves_typed_errors_and_refunds_before_payload_drop() {
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            panic!("payload destructor");
        }
    }
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1000);
    budget.reserve_storage(53).unwrap();
    let error: R<()> = scoped(&mut budget, |b| {
        b.reserve_storage(79)?;
        Err(E::History(LoopUnrollHistoryErrorV1::Mismatch(
            "retained typed child",
        )))
    });
    assert!(matches!(
        error,
        Err(E::History(LoopUnrollHistoryErrorV1::Mismatch(
            "retained typed child"
        )))
    ));
    assert_eq!(budget.storage(), 53);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: R<()> = scoped(&mut budget, |b| {
            b.reserve_storage(79)?;
            b.charge_work(3)?;
            std::panic::panic_any(Payload)
        });
    }));
    assert!(result.is_err());
    assert_eq!(
        (budget.storage(), budget.peak_storage(), budget.work()),
        (53, 132, 3)
    );
}

#[test]
fn final_u_nested_scope_refuses_foreign_ledger_without_credit_transfer() {
    let mut a = Work::new(100);
    let mut b = Work::new(100);
    let mut budget = Budget::new(&mut a, 1000);
    let mut foreign = Budget::new(&mut b, 1000);
    budget.reserve_storage(53).unwrap();
    foreign.reserve_storage(71).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut budget, |active| {
        active.reserve_storage(19)?;
        std::mem::swap(active, &mut foreign);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(E::Join(super::super::E::Resource(Resource::Accounting)))
    ));
    assert_eq!((budget.storage(), foreign.storage()), (71, 72));
    std::mem::swap(&mut budget, &mut foreign);
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget.release_storage(19).unwrap();
    assert_eq!((budget.storage(), foreign.storage()), (53, 71));
}

fn fixture_floor(fixture: &old::RootFixture) -> usize {
    fixture.source_storage
        + size_of::<old::RootFixture>()
        + fixture.fields.iter().map(Vec::capacity).sum::<usize>()
        + 2 * (fixture.middle.canonical_bytes().len()
            + fixture.descriptor.canonical_bytes().len()
            + fixture.native.canonical_bytes().len())
}

fn inputs(fixture: &old::RootFixture) -> Inputs<'_> {
    Inputs {
        semantic: fixture.source.semantic_ssa().source_semantic(),
        original: fixture.source.executable(),
        erased: None,
        launch: fixture.source.source_launch(),
        catalog: &fixture.catalog,
        middle: &fixture.middle,
        correspondence: &fixture.middle,
        verus: &fixture.middle,
    }
}

fn root_fields(fixture: &old::RootFixture) -> [Vec<u8>; 14] {
    let mut fields = fields();
    fields[13] = fixture.fields[13].clone();
    fields
}

fn roots_run(
    fixture: &old::RootFixture,
    fields: &[Vec<u8>; 14],
    w: usize,
    p: usize,
) -> (R<()>, usize, usize, Option<usize>) {
    let wire = framed(fields);
    let sibling = [0x71u8; 53];
    let mut work = Work::new(w);
    let mut budget = Budget::new(&mut work, p);
    budget.charge_work(11).unwrap();
    budget
        .reserve_storage(
            fixture_floor(fixture)
                + size_of::<Vec<u8>>()
                + wire.capacity()
                + size_of::<Inputs<'_>>()
                + size_of::<[Vec<u8>; 14]>()
                + fields.iter().map(Vec::capacity).sum::<usize>()
                + 53,
        )
        .unwrap();
    let frame = read(&wire, &mut budget);
    let input = inputs(fixture);
    let floor = budget.storage();
    let result = scoped(&mut budget, |b| {
        joins::roots(
            &frame,
            &input,
            fixture.source.executable(),
            &fixture.native,
            &fixture.descriptor,
            b,
        )
    });
    assert_eq!(budget.storage(), floor);
    assert_eq!(sibling, [0x71; 53]);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn final_u_root_components_keep_genuine_source_noncontiguous_ids_and_permutations() {
    let fixture = old::RootFixture::from_source(fixture_support::two_roots());
    assert_eq!(
        fixture
            .source
            .semantic_ssa()
            .source_semantic()
            .roots()
            .iter()
            .map(|v| v.index())
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(fixture.middle.canonical_kernel_order(), [1, 0]);
    let fields = root_fields(&fixture);
    let (result, work, peak, failed) = roots_run(&fixture, &fields, usize::MAX, MAX_STORAGE);
    result.unwrap();
    assert_eq!(failed, None);
    let exact = roots_run(&fixture, &fields, work, peak);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, peak, None));
    let short = roots_run(&fixture, &fields, work - 1, peak);
    match short.0 {
        Err(E::Join(super::super::E::Resource(Resource::Work(error)))) => {
            assert_eq!((error.actual(), error.limit()), (work, work - 1));
        }
        other => panic!("final symbol count charge: {other:?}"),
    }
    assert_eq!((short.1, short.2, short.3), (work - 1, peak, None));
}

#[test]
fn final_u_root_components_reject_foreign_source_and_each_source_launch_axis() {
    let fixture = old::RootFixture::from_source(fixture_support::plain_source(31));
    let donor = old::RootFixture::from_source(fixture_support::plain_source(32));
    assert!(matches!(
        roots_run(&fixture, &root_fields(&donor), usize::MAX, MAX_STORAGE).0,
        Err(E::Join(super::super::E::Mismatch(
            "exact semantic/N/F/descriptor root axes"
        )))
    ));
    let original = root_fields(&fixture);
    for offset in [8, 44, 92, 126, 138, 150, 158, 182, 206, 214] {
        let mut bad = original.clone();
        bad[13][offset] ^= 0x10;
        if offset == 214 {
            bad[13][offset] = original[13][offset] ^ 1;
        }
        assert!(
            matches!(
                roots_run(&fixture, &bad, usize::MAX, MAX_STORAGE).0,
                Err(E::Join(super::super::E::Mismatch(
                    "exact semantic/N/F/descriptor root axes"
                )))
            ),
            "offset {offset}"
        );
    }
    for (offset, expected) in [
        (80, "original function ordinal"),
        (88, "final function ordinal"),
    ] {
        let mut bad = original.clone();
        bad[13][offset..offset + 4].copy_from_slice(&19u32.to_le_bytes());
        assert!(
            matches!(roots_run(&fixture, &bad, usize::MAX, MAX_STORAGE).0,
            Err(E::Join(super::super::E::Mismatch(actual))) if actual == expected)
        );
    }
}

#[test]
fn final_u_root_components_check_all_three_permutation_axes_independently() {
    let fixture = old::RootFixture::from_source(fixture_support::two_roots());
    let original = root_fields(&fixture);
    let wire = framed(&original);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget
        .reserve_storage(
            fixture_floor(&fixture)
                + size_of::<Vec<u8>>()
                + wire.capacity()
                + size_of::<[Vec<u8>; 14]>()
                + original.iter().map(Vec::capacity).sum::<usize>(),
        )
        .unwrap();
    let frame = read(&wire, &mut budget);
    budget
        .reserve_storage(READ_STORAGE + 2 * size_of::<Root<'_>>())
        .unwrap();
    let first = frame
        .root(0, MAX_STORAGE, |n| budget.charge_work(n))
        .unwrap();
    let second = frame
        .root(1, MAX_STORAGE, |n| budget.charge_work(n))
        .unwrap();
    assert_eq!(
        [first.descriptor_ordinal, second.descriptor_ordinal],
        [1, 0]
    );
    assert_eq!(
        [
            first.original_kernel_ordinal,
            second.original_kernel_ordinal
        ],
        [0, 1]
    );
    assert_eq!(
        [first.final_kernel_ordinal, second.final_kernel_ordinal],
        [0, 1]
    );
    let second_start = 4 + 219 + first.logical_name.len() + first.export_name.len();
    for relative in [36, 72, 80] {
        let mut bad = original.clone();
        let a: [u8; 4] = bad[13][4 + relative..8 + relative].try_into().unwrap();
        let b: [u8; 4] = bad[13][second_start + relative..second_start + relative + 4]
            .try_into()
            .unwrap();
        bad[13][4 + relative..8 + relative].copy_from_slice(&b);
        bad[13][second_start + relative..second_start + relative + 4].copy_from_slice(&a);
        assert!(matches!(
            roots_run(&fixture, &bad, usize::MAX, MAX_STORAGE).0,
            Err(E::Join(super::super::E::Mismatch(
                "exact semantic/N/F/descriptor root axes"
            )))
        ));
    }
}

#[test]
fn final_u_native_envelope_components_keep_original_and_final_subjects_distinct() {
    use fe2o3_compiler_lineage::encode_native_neutral_module_v1;
    let original = old::RootFixture::from_source(fixture_support::plain_source(31));
    let other = old::RootFixture::from_source(fixture_support::plain_source(32));
    let a = original.source.executable();
    let b = other.source.executable();
    let a_subject = shared::subject(a, &original.catalog).unwrap();
    let b_subject = shared::subject(b, &other.catalog).unwrap();
    assert_ne!(a_subject, b_subject);
    let bytes = encode_native_neutral_module_v1(
        &a_subject,
        a.canonical().canonical_bytes(),
        original.catalog.canonical_bytes(),
    )
    .unwrap();
    let donor = encode_native_neutral_module_v1(
        &b_subject,
        b.canonical().canonical_bytes(),
        other.catalog.canonical_bytes(),
    )
    .unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    let floor = fixture_floor(&original)
        + fixture_floor(&other)
        + 2 * size_of::<Vec<u8>>()
        + bytes.capacity()
        + donor.capacity()
        + 2 * size_of::<super::super::Subject>();
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        super::super::envelope(&bytes, a, &original.catalog, &mut budget).unwrap(),
        a_subject
    );
    assert_eq!(
        super::super::envelope(&donor, b, &other.catalog, &mut budget).unwrap(),
        b_subject
    );
    for (wire, graph, catalog) in [
        (bytes.as_slice(), b, &other.catalog),
        (donor.as_slice(), a, &original.catalog),
        (bytes.as_slice(), a, &other.catalog),
    ] {
        assert!(matches!(
            super::super::envelope(wire, graph, catalog, &mut budget),
            Err(super::super::E::Mismatch("exact native envelope subject"))
        ));
    }
    assert_eq!(budget.storage(), floor);
}

fn native(
    profile: super::super::Profile,
    bitcode: bool,
    cov: fe2o3_compiler_ffi::CodeObjectVersion,
) -> Native {
    use fe2o3_compiler_ffi::{
        CompilerFfiEnvelopeV1, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
        CompilerModuleSymbolRoleV1, DeviceTargetV1,
    };
    let target = DeviceTargetV1::parse(profile.device_target()).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "entry"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "entry.kd"),
    ])
    .unwrap();
    let (kind, text): (_, &[u8]) = if bitcode {
        (CompilerModuleKindV1::LlvmBitcode, b"BC\xc0\xde")
    } else {
        (
            CompilerModuleKindV1::LlvmTextIr,
            b"define amdgpu_kernel void @entry() { ret void }\n",
        )
    };
    Native::new(
        kind,
        target,
        cov,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, cov).unwrap(),
        manifest,
        text,
    )
    .unwrap()
}

#[test]
fn final_u_shared_target_join_preserves_exact_profile_text_and_cov6_refusals() {
    use super::super::Profile;
    use fe2o3_compiler_ffi::CodeObjectVersion as Cov;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        assert_eq!(
            shared::profile(&native(profile, false, Cov::V6)).unwrap(),
            profile
        );
        for (bitcode, cov) in [(true, Cov::V6), (false, Cov::V5)] {
            assert!(matches!(
                shared::profile(&native(profile, bitcode, cov)),
                Err(super::super::E::Mismatch("LLVM text/COV6 native output"))
            ));
        }
    }
}

#[test]
fn final_u_v1_abi_components_preserve_all_scalar_slice_pointer_and_access_rules() {
    use fe2o3_kernel_descriptor::{
        AccessMode as A, ScalarTypeV1 as S, SourceTypeDescriptorV1 as D,
    };
    use fe2o3_kernel_ir::{AccessMode as K, AddressSpace as Space, ScalarType as S2, Type};
    for (source, actual) in [
        (S::I8, S2::I8),
        (S::U8, S2::U8),
        (S::I16, S2::I16),
        (S::U16, S2::U16),
        (S::I32, S2::I32),
        (S::U32, S2::U32),
        (S::I64, S2::I64),
        (S::U64, S2::U64),
        (S::F16, S2::F16),
        (S::F32, S2::F32),
        (S::F64, S2::F64),
    ] {
        let scalar = Type::Scalar(actual);
        assert!(shared::argument_kind(
            &D::scalar(source),
            &scalar,
            A::ByValue
        ));
        assert!(!shared::argument_kind(
            &D::scalar(source),
            &scalar,
            A::ReadOnly
        ));
        for access in [K::ReadOnly, K::WriteOnly, K::ReadWrite] {
            let slice = Type::slice(scalar.clone(), Space::Global, access);
            let pointer = Type::pointer(scalar.clone(), Space::Global, access);
            assert_eq!(
                shared::argument_kind(&D::shared_slice(source), &slice, A::ReadOnly),
                access == K::ReadOnly
            );
            assert_eq!(
                shared::argument_kind(&D::disjoint_slice(source), &slice, A::ReadWrite),
                access == K::ReadWrite
            );
            assert_eq!(
                shared::argument_kind(&D::disjoint_slice(source), &slice, A::WriteOnly),
                access == K::WriteOnly
            );
            assert_eq!(
                shared::argument_kind(&D::global_mut_pointer(source), &pointer, A::ReadWrite),
                access == K::ReadWrite
            );
            assert!(!shared::argument_kind(
                &D::shared_slice(source),
                &pointer,
                A::ReadOnly
            ));
            assert!(!shared::argument_kind(
                &D::global_mut_pointer(source),
                &slice,
                A::ReadWrite
            ));
        }
        for space in [Space::Private, Space::Workgroup] {
            assert!(!shared::argument_kind(
                &D::shared_slice(source),
                &Type::slice(scalar.clone(), space, K::ReadOnly),
                A::ReadOnly
            ));
            assert!(!shared::argument_kind(
                &D::global_mut_pointer(source),
                &Type::pointer(scalar.clone(), space, K::ReadWrite),
                A::ReadWrite
            ));
        }
        assert!(!shared::argument_kind(
            &D::scalar(source),
            &Type::INDEX,
            A::ByValue
        ));
    }
}

#[test]
fn final_u_capability_and_text_components_use_actual_graph_and_exact_output_bytes() {
    use fe2o3_amdgcn_model::{
        DescriptorCapabilityProjectionErrorV1 as Cap, NativeV12TextDescriptorReplayErrorV1 as Text,
    };
    let fixture = old::RootFixture::from_source(fixture_support::plain_source(31));
    let mut module = fixture.source.executable().module().clone();
    module
        .required_capabilities
        .insert(fe2o3_kernel_ir::TargetCapability::WorkgroupMemory);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    // Unadmitted module fixture construction uses the existing compiler-engine
    // domain. Fresh canonical owner and all continued controlled work are paid.
    let floor = fixture_floor(&fixture);
    budget.reserve_storage(floor).unwrap();
    let (actual, receipt) =
        Graph::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_ne!(
        actual.canonical().identity(),
        fixture.source.executable().canonical().identity()
    );
    shared::capabilities(
        fixture.source.executable(),
        &fixture.descriptor,
        &mut budget,
    )
    .unwrap();
    assert!(matches!(
        shared::capabilities(&actual, &fixture.descriptor, &mut budget),
        Err(super::super::E::Capabilities(Cap::CapabilityMismatch {
            descriptor: 0
        }))
    ));
    let result = check_native_v12_text_descriptor_relation_v1(
        &actual,
        &fixture.catalog,
        fixture.source.executable().canonical().canonical_bytes(),
        super::super::Profile::Gfx942,
        fixture.descriptor.table(),
        "",
        &mut budget,
    );
    assert!(matches!(result, Err(Text::OutputBytes)));
    drop(result);
    drop(actual);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn final_u_root_reader_work_denial_is_framing_charge_not_shared_join_failure() {
    let fields = fields();
    let wire = framed(&fields);
    let mut preparation = Work::new(usize::MAX);
    let mut prep = Budget::new(&mut preparation, MAX_STORAGE);
    prep.reserve_storage(
        size_of::<Vec<u8>>()
            + wire.capacity()
            + size_of::<[Vec<u8>; 14]>()
            + fields.iter().map(Vec::capacity).sum::<usize>(),
    )
    .unwrap();
    let frame = read(&wire, &mut prep);
    let mut work = Work::new(17);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.charge_work(17).unwrap();
    let floor = prep.storage() + READ_STORAGE + size_of::<Root<'_>>();
    budget.reserve_storage(floor).unwrap();
    let result = frame
        .root(0, MAX_STORAGE, |n| budget.charge_work(n))
        .map_err(E::Framing);
    match result {
        Err(E::Framing(FrameError::Charge(Resource::Work(error)))) => {
            assert_eq!(
                (error.actual(), error.limit()),
                (17 + 3 * fields[13].len(), 17)
            );
        }
        Err(other) => panic!("typed U reader callback refusal: {other:?}"),
        Ok(_) => panic!("missing U reader callback refusal"),
    }
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (17, floor, floor, None)
    );
}
