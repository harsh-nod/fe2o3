//! Exact request controls operate after genuine source/optimizer admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedOutputHandoffV26 as WorklistHandoff,
    ProductionConditionalMixedPureCseOutputHandoffV26 as PureCseHandoff,
    ProductionSourceOwnedViewV18 as Source,
};
use std::cell::Cell;

const REQUEST_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_cfg_tests::request_tests::mixed_cfg_request_child";
const WORK: usize = 500_000_000;
const STORAGE: usize = 20_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Measurement {
    work: usize,
    peak: usize,
}
#[derive(Clone, Copy)]
enum Bounds {
    Measure,
    Exact(Measurement),
    WorkShort(Measurement),
    StorageShort(Measurement),
}
thread_local! {
    static BOUNDS: Cell<Bounds> = const { Cell::new(Bounds::Measure) };
    static ENTERED: Cell<usize> = const { Cell::new(0) };
}

macro_rules! measured_request {
    ($name:ident, $handoff:ident, $prepare:ident) => {
        fn $name<'view, 'source>(
            source: &'view Source<'source>,
            handoff: &$handoff<'view, 'source>,
            _: &[AbiRoot<'_>],
            _: fe2o3_amd_target::ProductionAmdTargetProfileV1,
            budget: &mut Budget<'_>,
        ) -> Result<Measurement, Error> {
            ENTERED.with(|entered| entered.set(entered.get() + 1));
            let original_floor = budget.storage();
            let bounds = BOUNDS.with(Cell::get);
            // Start at the earlier peak so the measured request, not an old
            // optimizer scratch allocation, defines this storage boundary.
            let floor = match bounds {
                Bounds::Measure => budget.peak_storage(),
                Bounds::Exact(measured) | Bounds::WorkShort(measured) => {
                    budget.storage_limit() - measured.peak
                }
                Bounds::StorageShort(measured) => budget.storage_limit() - (measured.peak - 1),
            };
            let padding = floor.checked_sub(original_floor).unwrap();
            budget.reserve_storage(padding)?;
            if let Bounds::Exact(measured)
            | Bounds::WorkShort(measured)
            | Bounds::StorageShort(measured) = bounds
            {
                let allowed = measured.work - usize::from(matches!(bounds, Bounds::WorkShort(_)));
                budget.charge_work(WORK.checked_sub(budget.work()).unwrap() - allowed)?;
            }
            let before = budget.work();
            let result = match fe2o3_verifier::$prepare(source, handoff, budget) {
                Ok(request) => {
                    let inspected = (|| {
                        request.subject(budget)?;
                        request.generated_source(budget)?;
                        assert!(request.retained_storage(budget)? > 0);
                        Ok::<(), fe2o3_verifier::MixedOptimizerRefinementErrorV26>(())
                    })();
                    let settled = request.discard(budget);
                    inspected.and(settled).map_err(Error::ConditionalMixedCfg)
                }
                Err(error) => Err(Error::ConditionalMixedCfg(error)),
            };
            assert_eq!(
                budget.storage(),
                floor,
                "request credit must settle before caller padding"
            );
            let measured = Measurement {
                work: budget.work() - before,
                peak: budget.peak_storage() - floor,
            };
            budget.release_storage(padding)?;
            assert_eq!(budget.storage(), original_floor);
            result.map(|()| measured)
        }
    };
}
measured_request!(
    measure_worklist,
    WorklistHandoff,
    prepare_mixed_worklist_cfg_refinement_v27
);
measured_request!(
    measure_pure_cse,
    PureCseHandoff,
    prepare_mixed_pure_cse_cfg_refinement_v27
);

fn resource(error: &Error) -> Resource {
    let mut cause: &(dyn std::error::Error + 'static) = error;
    loop {
        if let Some(resource) = cause.downcast_ref::<Resource>() {
            return *resource;
        }
        if let Some(work) = cause.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>() {
            return Resource::Work(*work);
        }
        cause = cause
            .source()
            .unwrap_or_else(|| panic!("not a typed resource refusal: {error:?}"));
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestObservation {
    bounds: Vec<Measurement>,
    exact_short: usize,
    foreign_source: usize,
    foreign_ledger: usize,
    restored_floor: usize,
}
#[derive(Default)]
struct RequestCallbacks {
    result: Option<Result<RequestObservation, String>>,
}
impl Callbacks for RequestCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|error| format!("foreign genuine request source: {error:?}"))?;
            let mut report = RequestObservation {
                bounds: vec![],
                exact_short: 0,
                foreign_source: 0,
                foreign_ledger: 0,
                restored_floor: 0,
            };
            macro_rules! controls {
                ($route:ident, $limited:ident, $measure:ident, $prepare:ident) => {{
                    BOUNDS.with(|mode| mode.set(Bounds::Measure));
                    ENTERED.with(|entered| entered.set(0));
                    let measured = transaction()?.$limited(WORK, STORAGE, $measure)
                        .map_err(|error| format!("request measurement: {error:?}"))?.into_observation();
                    assert_eq!(ENTERED.with(Cell::get), 1);
                    assert!(measured.work > 0 && measured.peak > 0);
                    for bounds in [Bounds::Exact(measured), Bounds::WorkShort(measured), Bounds::StorageShort(measured)] {
                        BOUNDS.with(|mode| mode.set(bounds));
                        ENTERED.with(|entered| entered.set(0));
                        let actual = transaction()?.$limited(WORK, STORAGE, $measure);
                        assert_eq!(ENTERED.with(Cell::get), 1, "short limit must reach request preparation");
                        match bounds {
                            Bounds::Exact(_) => assert_eq!(actual.map_err(|error| format!("exact request: {error:?}"))?.into_observation(), measured),
                            Bounds::WorkShort(_) => assert!(matches!(resource(&refused(actual)), Resource::Work(error) if error.limit() == WORK && error.actual() > WORK)),
                            Bounds::StorageShort(_) => assert!(matches!(resource(&refused(actual)), Resource::Storage(error) if error.limit() == STORAGE && error.actual() > STORAGE)),
                            Bounds::Measure => unreachable!(),
                        }
                    }
                    report.bounds.push(measured);
                    report.exact_short += 1;

                    let source_called = Cell::new(false);
                    let error = refused(transaction()?.$route::<(), _>(|source, handoff, _, _, budget| {
                        let floor = budget.storage();
                        let request = fe2o3_verifier::$prepare(source, handoff, budget).map_err(Error::ConditionalMixedCfg)?;
                        let inspected = (|| {
                            request.check_original_source(source.source_ssa(budget)?, budget).map_err(Error::ConditionalMixedCfg)?;
                            let error = request.check_original_source(&foreign, budget).expect_err("equal-byte foreign SSA owner accepted");
                            assert!(matches!(error, fe2o3_verifier::MixedOptimizerRefinementErrorV26::Source(SourceError::Binding("foreign original SSA owner"))));
                            source_called.set(true);
                            Err::<(), _>(Error::ConditionalMixedCfg(error))
                        })();
                        let _ = request.discard(budget);
                        assert_eq!(budget.storage(), floor, "binding refusal preserves intact cleanup custody");
                        inspected
                    }));
                    assert!(source_called.get());
                    assert!(matches!(error, Error::ConditionalMixedCfg(fe2o3_verifier::MixedOptimizerRefinementErrorV26::Source(SourceError::Binding("foreign original SSA owner")))));
                    report.foreign_source += 1;

                    for undercut in [false, true] {
                        let reached = Cell::new(false);
                        let error = refused(transaction()?.$route::<(), _>(|source, handoff, _, _, budget| {
                            let request = fe2o3_verifier::$prepare(source, handoff, budget).map_err(Error::ConditionalMixedCfg)?;
                            let required = budget.storage();
                            let result = if undercut {
                                budget.release_storage(1)?;
                                let result = request.subject(budget);
                                budget.reserve_storage(1)?;
                                result
                            } else {
                                let mut work = Work::new(WORK);
                                let mut foreign = Budget::new(&mut work, STORAGE);
                                foreign.reserve_storage(required)?;
                                let result = request.subject(&foreign);
                                assert_eq!(foreign.storage(), required);
                                result
                            };
                            let error = result.expect_err("request lost custody but query succeeded");
                            assert!(matches!(error, fe2o3_verifier::MixedOptimizerRefinementErrorV26::Source(SourceError::Resource(Resource::Accounting))));
                            assert!(request.subject(budget).is_err(), "restoration cannot clear first refusal");
                            let settled = request.discard(budget);
                            assert!(settled.is_err());
                            assert_eq!(budget.storage(), required, "lost custody must not refund restored balances");
                            reached.set(true);
                            Err(Error::ConditionalMixedCfg(error))
                        }));
                        assert!(reached.get());
                        assert!(matches!(resource(&error), Resource::Accounting));
                        if undercut { report.restored_floor += 1; } else { report.foreign_ledger += 1; }
                    }
                }};
            }
            controls!(
                with_original_source_conditional_mixed_worklist_v26,
                with_original_source_conditional_mixed_test_limits_v26,
                measure_worklist,
                prepare_mixed_worklist_cfg_refinement_v27
            );
            controls!(
                with_original_source_conditional_mixed_pure_cse_v26,
                with_original_source_conditional_mixed_pure_cse_test_limits_v27,
                measure_pure_cse,
                prepare_mixed_pure_cse_cfg_refinement_v27
            );
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess child: requires actual captured rustc arguments"]
fn mixed_cfg_request_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = RequestCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual request controls did not run");
    std::fs::write(
        env::var_os(RESULT).expect("request result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual request controls: {result:?}");
}

#[test]
#[ignore = "requires pinned rust-src and authentic AMD dependencies; does not execute Verus"]
fn actual_original_mixed_cfg_requests_preserve_exact_bounds_and_custody() {
    run_actual_sources::<RequestObservation>(
        &[("duplicate", DUPLICATE)],
        &[(0, 0)],
        REQUEST_CHILD,
        "MIXED_CFG_REQUEST_CONTROLS_V27",
        program,
        |_, _, _, report, _| {
            assert_eq!(report.bounds.len(), 2);
            assert_eq!(
                (
                    report.exact_short,
                    report.foreign_source,
                    report.foreign_ledger,
                    report.restored_floor
                ),
                (2, 2, 2, 2)
            );
        },
    );
}
