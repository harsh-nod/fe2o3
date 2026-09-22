use super::*;
use crate::collector::TypedArgumentListV1;
use crate::compiler_descriptor::DescriptorArgumentKindV1;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::fixtures;
use fe2o3_kernel_descriptor::{BuildEvidenceV1, EvidenceDigest, EvidenceIdentity, ScalarTypeV1};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};

fn expected_digest(binding: &[u8], graph: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/EXPANDED-PRODUCTION-FINAL-EXECUTABLE-ABI/V3\0");
    hash.update(2u64.to_le_bytes());
    hash.update((binding.len() as u64).to_le_bytes());
    hash.update(binding);
    hash.update((graph.len() as u64).to_le_bytes());
    hash.update(graph);
    hash.finalize().into()
}
fn historical(owner: &Expanded) -> UnrolledOwnerV1<'_> {
    match owner.prefix() {
        ProductionExpandedPrefixV1::Direct(v) => UnrolledOwnerV1::Direct(v),
        ProductionExpandedPrefixV1::Erased(v) => UnrolledOwnerV1::Erased(v),
    }
}

pub(crate) fn exercise_actual_final(
    owner: &Expanded,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let wire = produce_for_profile(owner, roots, profile, 64, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    let wire_storage = wire.capacity() + size_of::<Vec<u8>>();
    budget.reserve_storage(wire_storage).unwrap();
    let legacy =
        nominal_v3::test_support::produce(historical(owner), roots, profile, budget).unwrap();
    let legacy_storage = legacy.capacity() + size_of::<Vec<u8>>();
    budget.reserve_storage(legacy_storage).unwrap();
    assert_ne!(
        wire, legacy,
        "U domain is never relabeled expanded, including no-op subjects"
    );
    budget
        .reserve_storage(
            DESCRIPTOR_TABLE_VIEW_STORAGE_V3
                + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
                + fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3,
        )
        .unwrap();
    let table = decode_device_descriptor_table_v3(&wire, &mut |n| budget.charge_work(n)).unwrap();
    assert_eq!(table.kernel_count(), roots.len());
    let mut previous_binding = None;
    for index in 0..table.kernel_count() {
        let row = table.kernel(index, &mut |n| budget.charge_work(n)).unwrap();
        let binding = *row.kernel_id().as_bytes();
        assert!(previous_binding.is_none_or(|previous| previous < binding));
        previous_binding = Some(binding);
        let root = roots
            .iter()
            .find(|root| root.kernel_binding_bytes() == binding)
            .unwrap();
        let digest = expected_digest(
            row.kernel_id().as_bytes(),
            owner.output().canonical().canonical_bytes(),
        );
        assert_eq!(
            row.executable_ir_evidence(),
            BuildEvidenceV1::new(
                EvidenceIdentity::from_opaque_bytes(digest),
                EvidenceDigest::from_sha256_bytes(digest),
            )
        );
        assert_eq!(row.entry_name(), root.entry_symbol());
        let old = historical(owner).output().canonical().canonical_bytes();
        if old != owner.output().canonical().canonical_bytes() {
            let stale = expected_digest(row.kernel_id().as_bytes(), old);
            assert_ne!(digest, stale);
        }
    }
    drop(table);
    budget
        .release_storage(
            DESCRIPTOR_TABLE_VIEW_STORAGE_V3
                + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
                + fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3,
        )
        .unwrap();
    drop(legacy);
    budget.release_storage(legacy_storage).unwrap();
    drop(wire);
    budget.release_storage(wire_storage).unwrap();
    assert_eq!(budget.storage(), floor);

    for case in 0..8 {
        let mut hostile = fixtures::typed_roots(historical(owner).final_f());
        let expected = fixtures::hostile(&mut hostile, case);
        let result = produce_for_profile(owner, &hostile, profile, 64, budget);
        assert!(
            matches!(result, Err(E::Nominal(NominalDescriptorErrorV3::Descriptor(
            CompilerDescriptorError::ProductionDescriptorMismatch(actual)
        ))) if actual == expected),
            "exact source/ABI/root refusal {case}: {result:?}"
        );
        assert_eq!(budget.storage(), floor);
    }
    let mut nominal = fixtures::typed_roots(historical(owner).final_f());
    let mut arguments = nominal[0].arguments.as_slice().to_vec();
    let index = arguments
        .iter()
        .position(|argument| argument.kind == DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64))
        .unwrap();
    arguments[index].kind = DescriptorArgumentKindV1::CompilerLaidOutUsize;
    nominal[0].arguments = TypedArgumentListV1::new(arguments).unwrap();
    assert!(matches!(
        produce_for_profile(owner, &nominal, profile, 64, budget),
        Err(E::Nominal(NominalDescriptorErrorV3::Mismatch(
            "actual rustc nominal kind"
        )))
    ));
    assert_eq!(budget.storage(), floor);
    let wrong = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => ProductionAmdTargetProfileV1::Gfx950,
        ProductionAmdTargetProfileV1::Gfx950 => ProductionAmdTargetProfileV1::Gfx942,
    };
    assert!(matches!(
        produce_for_profile(owner, roots, wrong, 64, budget),
        Err(E::Nominal(NominalDescriptorErrorV3::Descriptor(
            CompilerDescriptorError::CheckedOutputTarget(_)
        )))
    ));
    budget.reserve_storage(VIEW).unwrap();
    let mut view = original_view(owner).unwrap();
    view.kernels = &[];
    assert!(nominal_v3::validate_view(roots, &view, profile, budget).is_err());
    drop(view);
    budget.release_storage(VIEW).unwrap();
    assert_eq!(budget.storage(), floor);
}

pub(crate) fn exercise_common_transport(
    owner: &Expanded,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    llvm: &str,
    budget: &mut Budget<'_>,
) {
    use crate::kernel_ir_codegen::nominal_v3 as module;
    use dialect_amdgcn::NativeV12TextDescriptorReplayErrorV3 as NativeError;
    use fe2o3_compiler_ffi::{
        COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3, COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
        COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3, CompilerDescriptorSourceV3,
        compiler_descriptor_source_validation_storage_v3,
    };
    let floor = budget.storage();
    let wire = produce_for_profile(owner, roots, profile, 64, budget).unwrap();
    let pointer = wire.as_ptr();
    let capacity = wire.capacity();
    budget
        .reserve_storage(
            capacity
                + size_of::<Vec<u8>>()
                + COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3
                + COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3,
        )
        .unwrap();
    let source = CompilerDescriptorSourceV3::from_owned_canonical_bytes(
        wire,
        compiler_descriptor_source_validation_storage_v3(capacity).unwrap(),
        &mut |n| budget.charge_work(n),
    )
    .unwrap();
    budget
        .release_storage(size_of::<Vec<u8>>() + COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)
        .unwrap();
    assert_eq!(source.canonical_bytes().as_ptr(), pointer);
    assert!(!source.authenticates_compiler_origin());
    let (mut module, receipt) =
        module::retain_nominal_compiler_module_text_v3(owner.output(), llvm, &source, budget)
            .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    module::check_nominal_compiler_module_metadata_v3(owner.output(), &module, &source, budget)
        .unwrap();
    assert!(module.llvm_ir().starts_with(llvm));
    assert_eq!(module.llvm_ir().matches(".section .fe2o3.kd.v3").count(), 1);
    let check = |text: &str, budget: &mut Budget<'_>| {
        budget
            .reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
            .unwrap();
        let table = source
            .table(
                source.storage().retained_storage() + COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
                &mut |n| budget.charge_work(n),
            )
            .unwrap();
        let result = fe2o3_lower_mir_kernel::with_checked_source_pipeline_catalog_v1(
            owner.source_anchor(),
            budget,
            |view, budget| {
                let catalog = view.catalog(budget).map_err(NativeError::Resource)?;
                let relation = dialect_amdgcn::check_native_v12_text_descriptor_relation_v3(
                    owner.output(),
                    catalog,
                    owner.output().canonical().canonical_bytes(),
                    profile,
                    &table,
                    text,
                    budget,
                )?;
                budget
                    .reserve_storage(relation.storage().retained_storage())
                    .map_err(NativeError::Resource)?;
                assert_eq!(relation.pre_descriptor_llvm(), llvm);
                drop(relation);
                Ok::<_, NativeError>(())
            },
        );
        drop(table);
        budget
            .release_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
            .unwrap();
        result
    };
    check(module.llvm_ir(), budget).unwrap();
    let old = module.replace_first_ascii_byte_for_test_v3(b'!');
    assert!(check(module.llvm_ir(), budget).is_err());
    module.replace_first_ascii_byte_for_test_v3(old);
    check(module.llvm_ir(), budget).unwrap();
    drop(module);
    budget.release_storage(receipt.retained_storage()).unwrap();
    let bytes = source.storage().retained_storage();
    drop(source);
    budget.release_storage(bytes).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn expanded_domain_is_distinct_and_errors_keep_typed_causes() {
    use std::error::Error as _;
    assert_ne!(FINAL_DOMAIN, nominal_v3::FINAL_U_DOMAIN);
    for error in [E::Mismatch("test"), E::Panicked] {
        assert!(error.source().is_none());
    }
    assert!(
        E::Resource(Resource::Accounting)
            .source()
            .unwrap()
            .is::<Resource>()
    );
    assert!(
        E::Expanded(ProductionExpandedPolicyErrorV1::Policy)
            .source()
            .unwrap()
            .is::<ProductionExpandedPolicyErrorV1>()
    );
    assert!(
        E::Nominal(NominalDescriptorErrorV3::UnsupportedRequirements)
            .source()
            .unwrap()
            .is::<NominalDescriptorErrorV3>()
    );
}

#[test]
fn paid_scope_has_independent_exact_header_and_cleanup_oracles() {
    let guard = SCOPE + size_of::<R<()>>();
    for panic in [false, true] {
        let mut work = Work::new(20);
        let mut budget = Budget::new(&mut work, 29 + guard + 19);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(29).unwrap();
        let result: R<()> = scoped(
            &mut budget,
            || E::Panicked,
            |b| {
                b.charge_work(3)?;
                b.reserve_storage(19)?;
                if panic {
                    panic!("ordinary expanded scope unwind");
                }
                Ok(())
            },
        );
        assert_eq!(result.is_ok(), !panic);
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (20, 29, 29 + guard + 19)
        );
        assert_eq!(budget.failed_storage(), None);
    }
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 29 + guard - 1);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(29).unwrap();
    let result: R<()> = scoped(
        &mut budget,
        || E::Panicked,
        |_| panic!("header denied before body"),
    );
    assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (17, 29, 29)
    );
    assert_eq!(budget.failed_storage(), Some(29 + guard));
}

#[test]
fn rejected_scope_result_destructor_unwinds_after_dependents_drop_and_floor_refund() {
    struct PanicPayload;
    impl Drop for PanicPayload {
        fn drop(&mut self) {
            panic!("delayed rejected-result panic payload destructor");
        }
    }
    struct Payload(bool);
    impl Drop for Payload {
        fn drop(&mut self) {
            if self.0 {
                std::panic::panic_any(PanicPayload);
            }
            panic!("rejected result destructor");
        }
    }
    for hostile_panic_payload in [false, true] {
        let mut work = Work::new(128);
        let mut budget = Budget::new(&mut work, 4096);
        budget.reserve_storage(29).unwrap();
        let outside = catch_unwind(AssertUnwindSafe(|| {
            let result: R<Payload> = scoped(
                &mut budget,
                || E::Panicked,
                |b| {
                    let excess = b.storage() - 29;
                    b.release_storage(excess)?;
                    Ok(Payload(hostile_panic_payload))
                },
            );
            assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
        }));
        assert_eq!(outside.is_err(), hostile_panic_payload);
        assert_eq!(budget.storage(), 29);
    }
}
