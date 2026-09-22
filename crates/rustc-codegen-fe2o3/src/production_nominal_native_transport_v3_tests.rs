//! Constructed live N/E/F/U components; the separate rustc parent tests the
//! genuine consuming transaction. No synthetic final-owner constructor exists.
use super::*;
use crate::kernel_ir_codegen::{InertCompilerModuleTextV1, nominal_v3 as module};
use dialect_amdgcn::NativeV12TextDescriptorReplayErrorV3 as ModelError;
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3, COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3,
    CompilerDescriptorSourceV3 as Source,
};
use fe2o3_lower_mir_kernel::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, with_checked_source_pipeline_catalog_v1,
};
use native::descriptor_transport::{
    NominalLoopUnrollDescriptorTransportV3 as Transport,
    NominalNativeTransportErrorV3 as TransportError, test_support,
};

#[test]
fn nominal_transport_v3_addition_credits_old_vec_and_embedded_module_header_once() {
    native::test_custody_layout_matches_original_v3();
    let old = size_of::<native::NominalLoopUnrollNativeProductionCompilationV3>();
    let new = size_of::<Transport>();
    let header = size_of::<InertCompilerModuleTextV1>();
    assert!(new > old + header);
    for payload in [0usize, 1, 113, 4096] {
        assert_eq!(
            test_support::addition(header + payload).unwrap(),
            (new - old) + payload
        );
        assert_eq!(
            test_support::addition(header + payload).unwrap(),
            (new - old - header) + (header + payload)
        );
    }
    assert!(matches!(
        test_support::addition(header - 1),
        Err(TransportError::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        test_support::addition(usize::MAX),
        Err(TransportError::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn nominal_transport_v3_prepared_direct_erased_profiles_selected_and_noop_use_real_catalogs() {
    for erased in [false, true] {
        for profile in PROFILES {
            for bound in [Some(3), None] {
                with_prepared(erased, profile, bound, |native, budget| {
                    let floor = budget.storage();
                    let actual = native.owner.descriptor();
                    let roots = fixtures::typed_roots(actual.final_f());
                    let wire = support::produce(actual, &roots, profile, budget).unwrap();
                    let pointer = wire.as_ptr();
                    let capacity = wire.capacity();
                    let extent =
                        fe2o3_compiler_ffi::compiler_descriptor_source_validation_storage_v3(
                            capacity,
                        )
                        .unwrap();
                    let source_storage =
                        fe2o3_compiler_ffi::compiler_descriptor_source_retained_storage_v3(
                            capacity,
                        )
                        .unwrap()
                        .retained_storage();
                    budget
                        .reserve_storage(
                            source_storage + COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3,
                        )
                        .unwrap();
                    let source = Source::from_owned_canonical_bytes(wire, extent, &mut |n| {
                        budget.charge_work(n)
                    })
                    .unwrap();
                    budget
                        .release_storage(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)
                        .unwrap();
                    assert_eq!(source.canonical_bytes().as_ptr(), pointer);
                    let (module, receipt) = module::retain_nominal_compiler_module_text_v3(
                        native.output(),
                        native.llvm_ir(),
                        &source,
                        budget,
                    )
                    .unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(module.descriptor_binding_version_for_test_v3(), Some(3));
                    let anchor = match &native.owner {
                        Unrolled::Direct(v) => Anchor::Direct(
                            v.prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .source_semantic_kir(),
                        ),
                        Unrolled::Erased(v) => Anchor::Erased(
                            v.prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .prefix()
                                .erased_source(),
                        ),
                    };
                    let full_floor = budget.storage();
                    budget
                        .reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
                        .unwrap();
                    let table = source
                        .table(
                            source_storage + COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
                            &mut |n| budget.charge_work(n),
                        )
                        .unwrap();
                    let result =
                        with_checked_source_pipeline_catalog_v1(anchor, budget, |view, budget| {
                            let catalog = view.catalog(budget).map_err(ModelError::Resource)?;
                            let relation =
                                dialect_amdgcn::check_native_v12_text_descriptor_relation_v3(
                                    native.output(),
                                    catalog,
                                    native.output().canonical().canonical_bytes(),
                                    profile,
                                    &table,
                                    module.llvm_ir(),
                                    budget,
                                )?;
                            let retained = relation.storage().retained_storage();
                            budget
                                .reserve_storage(retained)
                                .map_err(ModelError::Resource)?;
                            assert!(!relation.grants_authority());
                            assert_eq!(relation.pre_descriptor_llvm(), native.llvm_ir());
                            drop(relation);
                            budget
                                .release_storage(retained)
                                .map_err(ModelError::Resource)?;
                            Ok::<_, ModelError>(())
                        });
                    result.unwrap();
                    drop(table);
                    budget
                        .release_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
                        .unwrap();
                    assert_eq!(budget.storage(), full_floor);
                    native.verify_equivalence(budget).unwrap();
                    module::check_nominal_compiler_module_metadata_v3(
                        native.output(),
                        &module,
                        &source,
                        budget,
                    )
                    .unwrap();
                    drop(module);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    drop(source);
                    budget.release_storage(source_storage).unwrap();
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }
}

#[test]
fn nominal_transport_v3_original_owner_scope_and_all_typed_causes_stay_visible() {
    use std::error::Error;
    macro_rules! child {
        ($value:expr,$kind:ty) => {{
            let value = $value;
            assert!(value.source().unwrap().is::<$kind>());
        }};
    }
    child!(TransportError::Resource(Resource::Accounting), Resource);
    child!(
        TransportError::Source(fe2o3_compiler_ffi::CompilerDescriptorSourceErrorV3::Work(
            Resource::Arithmetic
        )),
        fe2o3_compiler_ffi::CompilerDescriptorSourceErrorV3<Resource>
    );
    child!(TransportError::Nominal(E::Mismatch("source replay")), E);
    child!(
        TransportError::Module(module::NominalModuleErrorV3::Metadata("V3 tag")),
        module::NominalModuleErrorV3
    );
    child!(
        TransportError::Catalog(
            fe2o3_lower_mir_kernel::SourcePipelineCatalogCallbackErrorV1::Callback(
                ModelError::OutputBytes
            )
        ),
        fe2o3_lower_mir_kernel::SourcePipelineCatalogCallbackErrorV1<ModelError>
    );
    for error in [
        TransportError::Mismatch("component"),
        TransportError::Panicked,
    ] {
        assert!(error.source().is_none());
    }
}

#[test]
fn nominal_transport_v3_independent_first_header_and_error_unwind_cleanup() {
    let header = 2 * size_of::<usize>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>();
    assert_eq!(test_support::scope_header(), header);
    let header = header + size_of::<std::result::Result<(), TransportError>>();
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, 53 + header - 1);
    b.reserve_storage(53).unwrap();
    b.charge_work(17).unwrap();
    let result = test_support::scope(&mut b, |_| Ok(()));
    let Err(TransportError::Resource(Resource::Storage(e))) = result else {
        panic!("independent first header")
    };
    assert_eq!((e.actual(), e.limit()), (53 + header, 53 + header - 1));
    assert_eq!((b.work(), b.storage(), b.peak_storage()), (17, 53, 53));
    drop(b);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(53).unwrap();
    let result: std::result::Result<(), _> = test_support::scope(&mut b, |b| {
        b.reserve_storage(79)?;
        panic!("transport component panic")
    });
    assert!(matches!(result, Err(TransportError::Panicked)));
    assert_eq!(b.storage(), 53);
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("rejected result destructor")
        }
    }
    assert!(matches!(
        test_support::scope(&mut b, |b| {
            b.release_storage(1)?;
            Ok(Bomb)
        }),
        Err(TransportError::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), 53);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: std::result::Result<(), _> = test_support::scope(&mut b, |b| {
            b.reserve_storage(79)?;
            std::panic::panic_any(Bomb)
        });
    }));
    assert!(caught.is_err());
    assert_eq!(b.storage(), 53);
    test_support::scope(&mut b, |_| Ok(())).unwrap();
}

#[test]
fn nominal_transport_v3_replacement_ledger_is_not_refunded_or_repaired() {
    let mut work = Work::new(WORK);
    let mut other_work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let mut other = Budget::new(&mut other_work, STORAGE);
    b.reserve_storage(53).unwrap();
    b.charge_work(17).unwrap();
    other.reserve_storage(71).unwrap();
    other.charge_work(29).unwrap();
    let identity = other.work_ledger_identity_v1();
    let result = test_support::scope(&mut b, |b| {
        std::mem::swap(b, &mut other);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(TransportError::Resource(Resource::Accounting))
    ));
    assert!(b.work_ledger_identity_v1() == identity);
    assert_eq!((b.storage(), b.work()), (71, 29));
    assert_eq!(
        (other.storage(), other.work()),
        (
            53 + test_support::scope_header()
                + size_of::<std::result::Result<(), TransportError>>(),
            17
        )
    );
}

fn private_native_resource_samples_v1() -> [Resource; 5] {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(5);
    let mut budget = Budget::new(&mut work, 11);
    budget.reserve_storage(7).unwrap();
    let storage = budget.reserve_storage(5).unwrap_err();
    let Resource::Storage(storage_limit) = storage else {
        panic!("real storage denial");
    };
    assert_eq!((storage_limit.actual(), storage_limit.limit()), (12, 11));
    assert_eq!(
        (
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (7, 7, Some(12))
    );
    budget.charge_work(3).unwrap();
    let work = budget.charge_work(3).unwrap_err();
    let Resource::Work(work_limit) = work else {
        panic!("real work denial");
    };
    assert_eq!((work_limit.actual(), work_limit.limit()), (6, 5));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (3, 7, 7)
    );
    budget.release_storage(7).unwrap();
    assert_eq!(budget.storage(), 0);
    [
        work,
        storage,
        Resource::Accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ]
}

fn private_native_resource_from_chain_v1(
    mut error: &(dyn std::error::Error + 'static),
) -> Option<Resource> {
    for _ in 0..16 {
        if let Some(resource) = error.downcast_ref::<Resource>() {
            return Some(*resource);
        }
        error = error.source()?;
    }
    panic!("unexpected cyclic or excessively deep error chain");
}

fn nominal_transport_private_native_error_v1(
    error: crate::production_pipeline::private_cell_native_v1::PrivateCellNativeStageErrorV1,
) -> TransportError {
    TransportError::Nominal(
        crate::compiler_descriptor::nominal_v3::NominalDescriptorErrorV3::Pipeline(
            crate::production_pipeline::ProductionPipelineError::PrivateCellNativeStage(error),
        ),
    )
}

#[test]
fn nominal_transport_v3_private_native_direct_resource_causes_are_borrowed() {
    use crate::production_pipeline::private_cell_native_v1::PrivateCellNativeStageErrorV1 as Stage;
    use std::error::Error;

    for expected in private_native_resource_samples_v1() {
        let stage = Stage::Resource(expected);
        let actual = stage.source().unwrap().downcast_ref::<Resource>().unwrap();
        let Stage::Resource(stored) = &stage else {
            unreachable!();
        };
        assert!(std::ptr::eq(actual, stored));
        assert_eq!(*actual, expected);
        match expected {
            Resource::Work(limit) => assert_eq!(
                actual
                    .source()
                    .unwrap()
                    .downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>(),
                Some(&limit)
            ),
            Resource::Storage(limit) => assert_eq!(
                actual
                    .source()
                    .unwrap()
                    .downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrVerificationStorageLimitV1>(),
                Some(&limit)
            ),
            Resource::Accounting | Resource::Allocation | Resource::Arithmetic => {
                assert!(actual.source().is_none());
            }
        }
    }
}

#[test]
fn nominal_transport_v3_private_native_nested_resource_chain_preserves_exact_cause() {
    use crate::compiler_descriptor::nominal_v3::NominalDescriptorErrorV3 as Descriptor;
    use crate::production_pipeline::{
        ProductionPipelineError, private_cell_native_v1::PrivateCellNativeStageErrorV1 as Stage,
    };
    use std::error::Error;

    for expected in private_native_resource_samples_v1() {
        let outer = nominal_transport_private_native_error_v1(Stage::Resource(expected));
        let descriptor = outer.source().unwrap();
        assert!(descriptor.is::<Descriptor>());
        let pipeline = descriptor.source().unwrap();
        assert!(pipeline.is::<ProductionPipelineError>());
        let stage = pipeline.source().unwrap();
        assert!(stage.is::<Stage>());
        let resource = stage.source().unwrap();
        assert_eq!(resource.downcast_ref::<Resource>(), Some(&expected));
        assert_eq!(
            private_native_resource_from_chain_v1(&outer),
            Some(expected)
        );
    }
}

#[test]
fn nominal_transport_v3_private_native_nonresource_causes_remain_distinct() {
    use crate::production_pipeline::private_cell_native_v1::PrivateCellNativeStageErrorV1 as Stage;
    use fe2o3_lower_mir_kernel::ProductionPrivateCellPromotionContinuationErrorV1 as Admission;
    use std::error::Error;

    let mismatch = Stage::Mismatch("exact promoted native LLVM");
    assert!(mismatch.source().is_none());
    let outer = nominal_transport_private_native_error_v1(mismatch);
    assert_eq!(private_native_resource_from_chain_v1(&outer), None);

    let admission = Stage::Admission(Box::new(Admission::Panicked));
    let cause = admission.source().unwrap();
    let Stage::Admission(stored) = &admission else {
        unreachable!();
    };
    assert!(std::ptr::eq(
        cause.downcast_ref::<Admission>().unwrap(),
        stored.as_ref()
    ));
    assert!(matches!(
        cause.downcast_ref::<Admission>(),
        Some(Admission::Panicked)
    ));
    let outer = nominal_transport_private_native_error_v1(admission);
    assert_eq!(private_native_resource_from_chain_v1(&outer), None);
}
