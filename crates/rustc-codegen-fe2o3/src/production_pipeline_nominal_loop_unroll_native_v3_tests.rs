//! Constructed Direct/Erased components; only the rustc child claims source capture.
use super::super::nominal_v3 as native;
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::{
    fixtures,
    loop_unroll_v1::nominal_v3::{self as descriptor, test_support as support},
};
use crate::compiler_descriptor::nominal_v3::{
    NominalDescriptorErrorV3 as E, scoped as nominal_scope,
};

fn retain(wire: &[u8], capacity: usize, budget: &mut Budget<'_>) -> usize {
    assert!(!wire.is_empty());
    let storage = size_of::<Vec<u8>>() + capacity;
    budget.reserve_storage(storage).unwrap();
    storage
}

#[test]
fn nominal_unroll_native_errors_retain_every_typed_cause() {
    use std::error::Error as _;
    macro_rules! child {
        ($error:expr, $kind:ty) => {{
            let error = $error;
            assert!(error.source().unwrap().is::<$kind>());
        }};
    }
    child!(E::Resource(Resource::Accounting), Resource);
    child!(
        E::Descriptor(
            crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                "test"
            )
        ),
        crate::compiler_descriptor::CompilerDescriptorError
    );
    child!(
        E::Validation(fe2o3_kernel_descriptor::ValidationError::InvalidValue { field: "test" }),
        fe2o3_kernel_descriptor::ValidationError
    );
    child!(
        E::Wire(fe2o3_kernel_descriptor::DescriptorWireErrorV3::Work(
            Resource::Arithmetic
        )),
        fe2o3_kernel_descriptor::DescriptorWireErrorV3<Resource>
    );
    child!(
        E::Pipeline(ProductionPipelineError::RustcLineageMismatch),
        ProductionPipelineError
    );
    child!(
        E::Agreement(fe2o3_verifier::NominalSourceAbiErrorV3::TargetLayout),
        fe2o3_verifier::NominalSourceAbiErrorV3
    );
    child!(
        E::Unroll(LoopUnrollDescriptorErrorV1::Panicked),
        LoopUnrollDescriptorErrorV1
    );
    for error in [E::UnsupportedRequirements, E::Mismatch("test"), E::Panicked] {
        assert!(error.source().is_none());
    }
}

#[test]
fn nominal_unroll_native_header_keeps_original_custody_and_distinct_v3_wire() {
    let header = native::test_wrapper_header_v3();
    assert_eq!(
        header
            + size_of::<PreparedLoopUnrollNativeOutputV1>()
            + size_of::<PreparedRefinedForwardingHistoryClaimsV1>(),
        size_of::<native::NominalLoopUnrollNativeProductionCompilationV3>()
    );
    assert!(header >= size_of::<AuthenticatedProductionBindings>() + size_of::<Vec<u8>>());
    assert_ne!(
        descriptor::FINAL_U_DOMAIN,
        b"FE2O3/NOMINAL-EXECUTABLE-ABI/V3\0"
    );
    support::v1_nominal_refusal();
}

#[test]
fn nominal_unroll_native_components_cover_direct_erased_profiles_selected_and_noop() {
    for erased in [false, true] {
        for profile in PROFILES {
            for bound in [Some(3), None] {
                with_prepared(erased, profile, bound, |owner, budget| {
                    let floor = budget.storage();
                    let roots = fixtures::typed_roots(owner.owner.descriptor().final_f());
                    let wire = support::produce(owner.owner.descriptor(), &roots, profile, budget)
                        .unwrap();
                    assert_eq!(budget.storage(), floor);
                    let retained = retain(&wire, wire.capacity(), budget);
                    support::assert_final_subject(owner.owner.descriptor(), &wire, budget).unwrap();
                    owner.verify_equivalence(budget).unwrap();
                    assert!(!owner.grants_artifact_or_launch_authority());
                    assert!(owner.llvm_ir().contains("define amdgpu_kernel"));
                    drop(wire);
                    budget.release_storage(retained).unwrap();
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }
}

#[test]
fn nominal_unroll_native_join_refuses_report_source_launch_abi_and_kind_substitutions() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |native, budget| {
                let floor = budget.storage();
                let owner = native.owner.descriptor();
                let mut roots = fixtures::typed_roots(owner.final_f());
                assert!(matches!(support::omitted_reports(owner, &roots, profile, budget),
                    Err(E::Descriptor(crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                        "complete ordered typed/source/output/formal root roster")))));
                assert!(matches!(
                    support::nominal_substitution(owner, &mut roots, profile, budget),
                    Err(E::Mismatch("actual rustc nominal kind"))
                ));
                for case in 0..8 {
                    let mut roots = fixtures::typed_roots(owner.final_f());
                    let expected = fixtures::hostile(&mut roots, case);
                    let result = support::produce(owner, &roots, profile, budget);
                    assert!(
                        matches!(result, Err(E::Descriptor(
                        crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(actual))) if actual == expected),
                        "exact actual-U descriptor refusal for case {case}"
                    );
                    assert_eq!(budget.storage(), floor);
                }
            });
        }
    }
}

#[test]
fn nominal_unroll_native_join_refuses_wrong_target_before_encoding() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |native, budget| {
                let floor = budget.storage();
                let roots = fixtures::typed_roots(native.owner.descriptor().final_f());
                let wrong = if profile == Profile::Gfx942 {
                    Profile::Gfx950
                } else {
                    Profile::Gfx942
                };
                assert!(matches!(
                    support::produce(native.owner.descriptor(), &roots, wrong, budget),
                    Err(E::Descriptor(
                        crate::compiler_descriptor::CompilerDescriptorError::CheckedOutputTarget(_)
                    ))
                ));
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn nominal_unroll_native_descriptor_exact_cumulative_resources_and_first_header() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |native, original| {
                let floor = original.storage();
                let roots = fixtures::typed_roots(native.owner.descriptor().final_f());
                let probe = |work_limit, storage_limit| {
                    let mut work = Work::new(work_limit);
                    let mut budget = Budget::new(&mut work, storage_limit);
                    budget.charge_work(17).unwrap();
                    budget.reserve_storage(floor).unwrap();
                    let result =
                        support::produce(native.owner.descriptor(), &roots, profile, &mut budget);
                    let result = result.map(drop);
                    assert_eq!(budget.storage(), floor);
                    (
                        result,
                        budget.work(),
                        budget.peak_storage(),
                        budget.failed_storage(),
                    )
                };
                let (full, w, p, failed) = probe(WORK, STORAGE);
                full.unwrap();
                assert_eq!(failed, None);
                assert!(w > 17 && p > floor);
                let (exact, ew, ep, failed) = probe(w, p);
                exact.unwrap();
                assert_eq!((ew, ep, failed), (w, p, None));
                let (short, accepted, _, failed) = probe(w - 1, p);
                assert!(short.is_err());
                assert_eq!((accepted, failed), (w - 1, None));
                let (short, sw, sp, failed) = probe(w, p - 1);
                assert!(short.is_err());
                assert_eq!(failed, Some(p));
                println!(
                    "NOMINAL_U_STORAGE_OBSERVATION erased={erased} profile={profile:?} floor={floor} work={w} peak={p} short_work={sw} prior_peak={sp} error={short:?}"
                );
                let (short, accepted, peak, failed) = probe(w, floor + support::FIRST_HEADER - 1);
                match short {
                    Err(E::Resource(Resource::Storage(e))) => assert_eq!(
                        (e.actual(), e.limit()),
                        (
                            floor + support::FIRST_HEADER,
                            floor + support::FIRST_HEADER - 1
                        )
                    ),
                    other => panic!("header-first typed refusal: {other:?}"),
                }
                assert_eq!(
                    (accepted, peak, failed),
                    (17, floor, Some(floor + support::FIRST_HEADER))
                );
            });
        }
    }
}

#[test]
fn nominal_unroll_native_descriptor_preserves_floor_on_callback_failure_and_unwind() {
    with_prepared(true, Profile::Gfx942, Some(3), |native, budget| {
        let roots = fixtures::typed_roots(native.owner.descriptor().final_f());
        let wire =
            support::produce(native.owner.descriptor(), &roots, Profile::Gfx942, budget).unwrap();
        let retained = retain(&wire, wire.capacity(), budget);
        let floor = budget.storage();
        let result: std::result::Result<(), E> =
            descriptor::check_table(native.owner.descriptor(), &wire, budget, |_, budget| {
                budget.reserve_storage(11)?;
                Err(E::Mismatch("observer refusal"))
            });
        assert!(matches!(result, Err(E::Mismatch("observer refusal"))));
        assert_eq!(budget.storage(), floor);
        let result: std::result::Result<(), E> =
            descriptor::check_table(native.owner.descriptor(), &wire, budget, |_, budget| {
                budget.reserve_storage(13)?;
                panic!("observer panic")
            });
        assert!(matches!(result, Err(E::Panicked)));
        assert_eq!(budget.storage(), floor);
        drop(wire);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn nominal_unroll_native_cleanup_restores_valid_floor_before_destructor_unwind() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("panic payload destructor");
        }
    }
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1024);
    budget.reserve_storage(53).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: std::result::Result<(), E> = nominal_scope(&mut budget, |budget| {
            budget.charge_work(19)?;
            budget.reserve_storage(61)?;
            std::panic::panic_any(Bomb);
        });
    }));
    assert!(caught.is_err());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (19, 53, 114)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn nominal_unroll_native_cleanup_never_refunds_foreign_ledger() {
    let mut work = Work::new(100);
    let mut foreign_work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1024);
    let mut foreign = Budget::new(&mut foreign_work, 1024);
    budget.reserve_storage(53).unwrap();
    foreign.reserve_storage(37).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let foreign_ledger = foreign.work_ledger_identity_v1();
    let result: std::result::Result<(), E> = nominal_scope(&mut budget, |budget| {
        budget.charge_work(19)?;
        budget.reserve_storage(61)?;
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert!(budget.work_ledger_identity_v1() == foreign_ledger);
    assert!(foreign.work_ledger_identity_v1() == ledger);
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            foreign.work(),
            foreign.storage()
        ),
        (0, 37, 19, 114)
    );
    std::mem::swap(&mut budget, &mut foreign);
    budget.release_storage(61).unwrap();
    assert_eq!(budget.storage(), 53);
}
