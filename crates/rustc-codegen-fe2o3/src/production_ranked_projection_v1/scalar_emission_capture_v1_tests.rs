use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;
#[path = "scalar_emission_fixture_v1_tests.rs"]
mod fixture;
const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 23;

fn input() -> ProductionRankedRootInputV1 {
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
        fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    ProductionRankedRootInputV1::new("component", [30; 32], &launch)
}
fn capture(budget: &mut Budget<'_>) -> Capture {
    let (ssa, launch) = fixture::source(30);
    Capture::try_materialize_with_budget_v1(ssa, launch, Default::default(), budget).unwrap()
}
fn project(budget: &mut Budget<'_>) -> CapturedRankedSourceV1 {
    project_seed(budget, 30)
}
fn project_seed(budget: &mut Budget<'_>, seed: u8) -> CapturedRankedSourceV1 {
    let (ssa, launch) = fixture::source(seed);
    let capture =
        Capture::try_materialize_with_budget_v1(ssa, launch, Default::default(), budget).unwrap();
    budget
        .reserve_storage(capture.retained_analysis_storage_v1())
        .unwrap();
    let mut root = input();
    root.kernel_binding = [seed; 32];
    CapturedRankedSourceV1::try_project_v1(
        capture,
        &[root],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
        budget,
    )
    .unwrap()
}

fn resource_error(error: &(dyn std::error::Error + 'static)) -> Option<Resource> {
    if let Some(error) = error.downcast_ref::<Resource>() {
        return Some(*error);
    }
    // These public analysis errors intentionally do not all expose a source chain.
    if let Some(error) = error.downcast_ref::<CanonicalAssertionErrorV1>() {
        return match error {
            CanonicalAssertionErrorV1::Resource(error)
            | CanonicalAssertionErrorV1::Inventory(
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error),
            )
            | CanonicalAssertionErrorV1::Sparse(
                fe2o3_kernel_analysis::CanonicalKirSparseErrorV1::Resource(error),
            )
            | CanonicalAssertionErrorV1::MemorySsa(
                fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1::Resource(error),
            )
            | CanonicalAssertionErrorV1::CallEffects(
                fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1::Resource(error),
            )
            | CanonicalAssertionErrorV1::Origin(
                fe2o3_lower_mir_kernel::SemanticKirAssertOriginErrorV1::Resource(error),
            )
            | CanonicalAssertionErrorV1::PrivateArray(
                fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1::Resource(error),
            )
            | CanonicalAssertionErrorV1::MaskedAssertion(
                fe2o3_lower_mir_kernel::ProductionSemanticMaskedShiftQueryErrorV1::Resource(error),
            ) => Some(*error),
            _ => None,
        };
    }
    if let Some(error) = error.downcast_ref::<CaptureError>() {
        return match error {
            CaptureError::Resource(error)
            | CaptureError::Inventory(
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error),
            )
            | CaptureError::Loops(fe2o3_kernel_analysis::CanonicalKirLoopErrorV1::Resource(
                error,
            )) => Some(*error),
            _ => None,
        };
    }
    error.source().and_then(resource_error)
}
fn require_resource<T, E: std::error::Error + 'static>(result: Result<T, E>, storage: bool) {
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("one-short resource unexpectedly succeeded"),
    };
    match (resource_error(&error), storage) {
        (Some(Resource::Work(_)), false) | (Some(Resource::Storage(_)), true) => (),
        _ => panic!("wrong exact resource failure: {error:?}"),
    }
}

#[test]
fn actual_projection_retains_one_report_and_queries_same_source_n() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let stage = project(&mut budget);
    assert_eq!(budget.storage(), FLOOR);
    budget.reserve_storage(stage.retained_storage()).unwrap();
    let floor = budget.storage();
    let mut seen = 0;
    stage
        .with_observations_v1(&mut budget, |row, _| {
            assert_eq!(row.certificate_ordinal(), Some(0));
            assert!(std::ptr::eq(
                row.report(),
                &stage.roots[0].semantic_u32_induction
            ));
            assert_eq!(row.report().certificates().len(), 1);
            let Some(Consistency::Joined(fact)) = row.outcome() else {
                panic!("actual join absent")
            };
            assert!(std::ptr::eq(fact.source(), stage.capture().original()));
            assert_eq!(fact.certificate(), row.report().certificates()[0]);
            assert_eq!(fact.root(), row.root().semantic_root());
            assert!(!fact.authorizes_compiler_transform());
            seen += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, 1);
    assert_eq!(stage.root_count(), 1);
    assert!(!stage.grants_authority());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn inert_fact_may_escape_observation_without_retaining_scoped_analysis() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let stage = project(&mut setup);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + stage.retained_storage();
    budget.reserve_storage(floor).unwrap();
    let mut retained_fact = None;
    stage
        .with_observations_v1(&mut budget, |row, _| {
            retained_fact = row.outcome();
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
    let Some(Consistency::Joined(fact)) = retained_fact else {
        panic!("missing inert source fact")
    };
    assert!(std::ptr::eq(fact.source(), stage.capture().original()));
    assert!(!fact.authorizes_compiler_transform());
}

#[test]
fn root_binding_substitution_consumes_only_reserved_capture_not_unrelated_floor() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = capture(&mut budget);
    budget
        .reserve_storage(capture.retained_analysis_storage_v1())
        .unwrap();
    let mut changed = input();
    changed.kernel_binding = [31; 32];
    let result = CapturedRankedSourceV1::try_project_v1(
        capture,
        &[changed],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
        &mut budget,
    );
    assert!(matches!(result, Err(Error::RankedProjection(_))));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn projection_wrapper_and_query_have_exact_and_one_short_live_resources() {
    let run_project = |limit, capacity| {
        let mut setup_work = Work::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let capture = capture(&mut setup);
        let incoming = capture.retained_analysis_storage_v1();
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, capacity);
        budget.reserve_storage(FLOOR + incoming).unwrap();
        let result = CapturedRankedSourceV1::try_project_v1(
            capture,
            &[input()],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
            &mut budget,
        );
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    };
    let (stage, spent, peak) = run_project(WORK, STORAGE);
    let stage = stage.unwrap();
    run_project(spent, peak).0.unwrap();
    require_resource(run_project(spent - 1, peak).0, false);
    require_resource(run_project(spent, peak - 1).0, true);
    let run_query = |limit, capacity| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, capacity);
        let floor = FLOOR + stage.retained_storage();
        budget.reserve_storage(floor).unwrap();
        let result = stage.with_observations_v1(&mut budget, |_, _| Ok(()));
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, spent, peak) = run_query(WORK, STORAGE);
    result.unwrap();
    run_query(spent, peak).0.unwrap();
    require_resource(run_query(spent - 1, peak).0, false);
    require_resource(run_query(spent, peak - 1).0, true);
}

#[test]
fn capture_and_stage_require_their_complete_live_receipt_before_work() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let captured = capture(&mut setup);
    let stage = project(&mut setup);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(captured.retained_analysis_storage_v1() - 1)
        .unwrap();
    let before = (budget.work(), budget.storage());
    let result = CapturedRankedSourceV1::try_project_v1(
        captured,
        &[input()],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
        &mut budget,
    );
    assert!(matches!(result, Err(Error::ScalarEmissionCapture(error))
        if matches!(*error, CaptureError::Resource(Resource::Accounting))));
    assert_eq!((budget.work(), budget.storage()), before);
    budget.release_storage(budget.storage()).unwrap();
    budget
        .reserve_storage(stage.retained_storage() - 1)
        .unwrap();
    let before = (budget.work(), budget.storage());
    let result = stage.with_observations_v1(&mut budget, |_, _| {
        panic!("missing receipt reached callback")
    });
    assert!(matches!(
        result,
        Err(CaptureError::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        stage.observation_count_v1(&mut budget),
        Err(CaptureError::Resource(Resource::Accounting))
    ));
    assert_eq!((budget.work(), budget.storage()), before);
}

#[test]
fn substituted_actual_root_or_foreign_real_report_cannot_become_an_observation() {
    for report in [false, true] {
        let mut setup_work = Work::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let mut stage = project(&mut setup);
        if report {
            let mut foreign = project_seed(&mut setup, 31);
            std::mem::swap(
                &mut stage.roots[0].semantic_u32_induction,
                &mut foreign.roots[0].semantic_u32_induction,
            );
        } else {
            stage.roots[0].semantic_root = SemanticFunctionIdV1::from_index(1);
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + stage.retained_storage();
        budget.reserve_storage(floor).unwrap();
        let result = stage.with_observations_v1(&mut budget, |_, _| {
            panic!("foreign custody reached callback")
        });
        let expected = if report {
            "certificate actual source identity"
        } else {
            "certificate actual root/body alias"
        };
        assert!(matches!(result, Err(CaptureError::Mismatch(detail)) if detail == expected));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn observer_error_panic_and_extra_reservation_are_not_success() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let stage = project(&mut setup);
    for mode in 0..3 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + stage.retained_storage();
        budget.reserve_storage(floor).unwrap();
        let called = Cell::new(0);
        let result = stage.with_observations_v1(&mut budget, |_, budget| {
            called.set(called.get() + 1);
            match mode {
                0 => Err(CaptureError::Mismatch("observer")),
                1 => panic!("observer"),
                _ => {
                    budget.reserve_storage(7)?;
                    Ok(())
                }
            }
        });
        assert_eq!(called.get(), 1);
        match mode {
            0 => assert!(matches!(result, Err(CaptureError::Mismatch("observer")))),
            1 => assert!(matches!(result, Err(CaptureError::Panicked))),
            _ => assert!(matches!(
                result,
                Err(CaptureError::Resource(Resource::Accounting))
            )),
        }
        assert_eq!(budget.storage(), floor + if mode == 2 { 7 } else { 0 });
    }
}

#[test]
fn observer_replaced_ledger_or_undercut_floor_is_not_charged_or_released() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let stage = project(&mut setup);
    for replace in [false, true] {
        let mut original = Work::new(WORK);
        let mut foreign = Work::new(WORK);
        let mut budget = Budget::new(&mut original, STORAGE);
        let mut spare = Budget::new(&mut foreign, STORAGE);
        let floor = FLOOR + stage.retained_storage();
        budget.reserve_storage(floor).unwrap();
        spare.reserve_storage(floor).unwrap();
        let mut observed = (0, 0);
        let result = stage.with_observations_v1(&mut budget, |_, budget| {
            if replace {
                std::mem::swap(budget, &mut spare);
            } else {
                budget.release_storage(1)?;
            }
            observed = (budget.work(), budget.storage());
            Ok(())
        });
        assert!(matches!(
            result,
            Err(CaptureError::Resource(Resource::Accounting))
        ));
        assert_eq!((budget.work(), budget.storage()), observed);
    }
}
