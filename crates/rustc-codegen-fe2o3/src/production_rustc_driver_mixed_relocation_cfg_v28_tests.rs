use super::*;

const CFG_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_relocation_tests::cfg_tests::mixed_relocation_cfg_child";
const COMPOSED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_relocation_tests::cfg_tests::mixed_composed_relocation_cfg_child";
thread_local! {
    static COMPOSED: Cell<bool> = const { Cell::new(false) };
}

enum Request<'h, 'n, 'p, 'v, 's> {
    Tail(fe2o3_verifier::PreparedMixedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's>),
    Composed(fe2o3_verifier::PreparedMixedComposedRelocationCfgRefinementV28<'h, 'n, 'p, 'v, 's>),
}
macro_rules! request_query {
    ($self:ident, $method:ident $(, $argument:expr)*) => {
        match $self {
            Self::Tail(request) => request.$method($($argument),*),
            Self::Composed(request) => request.$method($($argument),*),
        }
    };
}
impl Request<'_, '_, '_, '_, '_> {
    fn subject(
        &self,
        budget: &Budget<'_>,
    ) -> Result<fe2o3_verifier::MixedOptimizerRelocationCfgSubjectV28, RequestError> {
        request_query!(self, subject, budget)
    }
    fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8], RequestError> {
        request_query!(self, generated_source, budget)
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result<(), RequestError> {
        request_query!(self, replay, budget)
    }
    fn check_original_source(
        &self,
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<(), RequestError> {
        request_query!(self, check_original_source, source, budget)
    }
    fn discard(self, budget: &mut Budget<'_>) -> Result<(), RequestError> {
        request_query!(self, discard, budget)
    }
    fn authenticates_executed_proof(&self) -> bool {
        request_query!(self, authenticates_executed_proof)
    }
    fn proves_original_to_final_composition(&self) -> bool {
        request_query!(self, proves_original_to_final_composition)
    }
    fn grants_artifact_or_launch_authority(&self) -> bool {
        request_query!(self, grants_artifact_or_launch_authority)
    }
}
fn prepare_request<'h, 'n, 'p, 'v, 's>(
    source: &'h Source<'s>,
    native: &'h fe2o3_lower_mir_kernel::ProductionConditionalMixedLicmOutputHandoffV28<
        'n,
        'p,
        'v,
        's,
    >,
    budget: &mut Budget<'_>,
) -> Result<Request<'h, 'n, 'p, 'v, 's>, RequestError> {
    let request = prepare(source, native, budget)?;
    if COMPOSED.with(Cell::get) {
        request
            .prepare_composed_cfg_refinement(budget)
            .map(Request::Composed)
    } else {
        request.prepare_cfg_refinement(budget).map(Request::Tail)
    }
}

fn measure_cfg<'p, 'v, 's>(
    source: &'v Source<'s>,
    relocation: &Relocation<'p, 'v, 's>,
    _: &[AbiRoot<'_>],
    _: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<Measurement, Error> {
    let native = relocation
        .complete_native_v28(budget)
        .map_err(Error::MixedLicmCompletion)?;
    let measured = (|| {
        ENTERED.with(|entered| entered.set(entered.get() + 1));
        let original = budget.storage();
        let bounds = BOUNDS.with(Cell::get);
        let floor = match bounds {
            Bounds::Measure => budget.peak_storage(),
            Bounds::Exact(measured) | Bounds::WorkShort(measured) => {
                budget.storage_limit() - measured.peak
            }
            Bounds::StorageShort(measured) => budget.storage_limit() - (measured.peak - 1),
        };
        let padding = floor.checked_sub(original).unwrap();
        budget.reserve_storage(padding)?;
        if let Bounds::Exact(measured)
        | Bounds::WorkShort(measured)
        | Bounds::StorageShort(measured) = bounds
        {
            let allowed = measured.work - usize::from(matches!(bounds, Bounds::WorkShort(_)));
            budget.charge_work(WORK - budget.work() - allowed)?;
        }
        let before = budget.work();
        let result = match prepare_request(source, &native, budget) {
            Ok(request) => {
                let inspected = (|| {
                    request.replay(budget)?;
                    let subject = request.subject(budget)?;
                    assert_eq!(
                        subject.models_original_to_final_composition(),
                        COMPOSED.with(Cell::get)
                    );
                    assert!(subject.modeled_functions() >= 2);
                    assert!(subject.modeled_blocks() > 2);
                    assert!(subject.expressions().moved_operations() >= 4);
                    assert_eq!(subject.expressions().slice_premises(), 4);
                    let generated = std::str::from_utf8(request.generated_source(budget)?).unwrap();
                    assert_eq!(
                        generated
                            .matches("proof fn relocation_function_trace_")
                            .count(),
                        subject.modeled_functions()
                    );
                    assert!(generated.contains("relocated_value_"));
                    assert_eq!(generated.contains("cfg_live_"), COMPOSED.with(Cell::get));
                    assert_eq!(
                        generated
                            .matches("proof fn composed_original_to_final_trace_")
                            .count(),
                        if COMPOSED.with(Cell::get) {
                            subject.modeled_functions()
                        } else {
                            0
                        }
                    );
                    assert!(!request.authenticates_executed_proof());
                    assert!(!request.proves_original_to_final_composition());
                    assert!(!request.grants_artifact_or_launch_authority());
                    Ok::<(), RequestError>(())
                })();
                let released = request.discard(budget);
                inspected.and(released)
            }
            Err(error) => Err(error),
        }
        .map_err(Error::MixedRelocationExpressions);
        assert_eq!(
            budget.storage(),
            floor,
            "failed or accepted CFG requests settle before caller padding"
        );
        let measurement = Measurement {
            work: budget.work() - before,
            peak: budget.peak_storage() - floor,
        };
        budget.release_storage(padding)?;
        assert_eq!(budget.storage(), original);
        result.map(|()| measurement)
    })();
    let released = native.discard(budget);
    selected(measured, released)
}

#[derive(Debug, Serialize, Deserialize)]
struct CfgObservation {
    composed: bool,
    measurement: Measurement,
    exact_short: usize,
    foreign_source: usize,
    custody: usize,
}
#[derive(Default)]
struct CfgCallbacks {
    composed: bool,
    result: Option<Result<CfgObservation, String>>,
}
impl Callbacks for CfgCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        COMPOSED.with(|mode| mode.set(self.composed));
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            BOUNDS.with(|mode| mode.set(Bounds::Measure));
            ENTERED.with(|count| count.set(0));
            let measurement = transaction()?
                .with_original_source_mixed_licm_test_limits_v28(WORK, STORAGE, measure_cfg)
                .map_err(|error| format!("relocation CFG measurement: {error:?}"))?
                .into_observation();
            assert_eq!(ENTERED.with(Cell::get), 1);
            for bounds in [
                Bounds::Exact(measurement),
                Bounds::WorkShort(measurement),
                Bounds::StorageShort(measurement),
            ] {
                BOUNDS.with(|mode| mode.set(bounds));
                ENTERED.with(|count| count.set(0));
                let result = transaction()?.with_original_source_mixed_licm_test_limits_v28(
                    WORK,
                    STORAGE,
                    measure_cfg,
                );
                assert_eq!(ENTERED.with(Cell::get), 1);
                match bounds {
                    Bounds::Exact(_) => assert_eq!(
                        result
                            .map_err(|error| format!("exact relocation CFG request: {error:?}"))?
                            .into_observation(),
                        measurement
                    ),
                    Bounds::WorkShort(_) => assert!(
                        matches!(resource(&refused(result)), Resource::Work(error) if error.limit() == WORK && error.actual() > WORK)
                    ),
                    Bounds::StorageShort(_) => assert!(
                        matches!(resource(&refused(result)), Resource::Storage(error) if error.limit() == STORAGE && error.actual() > STORAGE)
                    ),
                    Bounds::Measure => unreachable!(),
                }
            }
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|error| format!("foreign source: {error:?}"))?;
            let seen = Cell::new(false);
            let result = transaction()?.with_original_source_mixed_licm_v28::<(), _>(
                |source, relocation, _, _, budget| {
                    let native = relocation
                        .complete_native_v28(budget)
                        .map_err(Error::MixedLicmCompletion)?;
                    let result = (|| {
                        let floor = budget.storage();
                        let request = prepare_request(source, &native, budget)
                            .map_err(Error::MixedRelocationExpressions)?;
                        let rejected = request.check_original_source(&foreign, budget);
                        assert!(matches!(
                            rejected,
                            Err(RequestError::Source(SourceError::Binding(
                                "foreign original SSA owner"
                            )))
                        ));
                        let _ = request.discard(budget);
                        assert_eq!(budget.storage(), floor);
                        seen.set(true);
                        rejected.map_err(Error::MixedRelocationExpressions)
                    })();
                    let released = native.discard(budget);
                    selected(result, released)
                },
            );
            assert!(seen.get());
            assert!(matches!(
                refused(result),
                Error::MixedRelocationExpressions(RequestError::Source(SourceError::Binding(
                    "foreign original SSA owner"
                )))
            ));
            for undercut in [false, true] {
                let seen = Cell::new(false);
                let result = transaction()?.with_original_source_mixed_licm_v28::<(), _>(
                    |source, relocation, _, _, budget| {
                        let native = relocation
                            .complete_native_v28(budget)
                            .map_err(Error::MixedLicmCompletion)?;
                        let request = prepare_request(source, &native, budget)
                            .map_err(Error::MixedRelocationExpressions)?;
                        let required = budget.storage();
                        let denied = if undercut {
                            budget.release_storage(1)?;
                            let denied = request.generated_source(budget).map(|_| ());
                            budget.reserve_storage(1)?;
                            denied
                        } else {
                            let mut work = Work::new(WORK);
                            let mut foreign = Budget::new(&mut work, STORAGE);
                            foreign.reserve_storage(required)?;
                            let denied = request.generated_source(&foreign).map(|_| ());
                            assert_eq!(foreign.storage(), required);
                            denied
                        }
                        .expect_err("foreign or restored CFG custody");
                        assert!(matches!(
                            denied,
                            RequestError::Source(SourceError::Resource(Resource::Accounting))
                        ));
                        assert!(request.subject(budget).is_err());
                        assert!(request.discard(budget).is_err());
                        assert!(native.discard(budget).is_err());
                        assert_eq!(budget.storage(), required);
                        seen.set(true);
                        Err(Error::MixedRelocationExpressions(denied))
                    },
                );
                assert!(seen.get());
                assert!(matches!(resource(&refused(result)), Resource::Accounting));
            }
            Ok(CfgObservation {
                composed: COMPOSED.with(Cell::get),
                measurement,
                exact_short: 3,
                foreign_source: 1,
                custody: 2,
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess child requires actual rustc arguments"]
fn mixed_relocation_cfg_child() {
    run_child(false);
}

#[test]
#[ignore = "subprocess child requires actual rustc arguments"]
fn mixed_composed_relocation_cfg_child() {
    run_child(true);
}

fn run_child(composed: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = CfgCallbacks {
        composed,
        ..CfgCallbacks::default()
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual CFG request callback");
    std::fs::write(
        env::var_os(RESULT).expect("CFG result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual relocation CFG request: {result:?}");
}

#[test]
#[ignore = "requires pinned rust-src/rustc-dev and actual AMD dependencies; no Verus execution"]
fn actual_original_relocation_cfg_requests_keep_final_graph_and_exact_custody() {
    run_actual_sources::<CfgObservation>(
        &[("loop", LOOP), ("nested", NESTED)],
        &[(0, 0)],
        CFG_CHILD,
        "MIXED_RELOCATION_CFG_V28",
        program,
        |_, _, _, report, _| {
            assert!(!report.composed);
            assert_eq!(
                (report.exact_short, report.foreign_source, report.custody),
                (3, 1, 2)
            );
            assert!(report.measurement.work > 0 && report.measurement.peak > 0);
        },
    );
}

#[test]
#[ignore = "requires pinned rust-src/rustc-dev and actual AMD dependencies; no Verus execution"]
fn actual_original_composed_relocation_cfg_requests_join_all_three_graphs_and_exact_custody() {
    run_actual_sources::<CfgObservation>(
        &[("loop", LOOP), ("nested", NESTED)],
        &[(0, 0)],
        COMPOSED_CHILD,
        "MIXED_COMPOSED_RELOCATION_CFG_V28",
        program,
        |_, _, _, report, _| {
            assert!(report.composed);
            assert_eq!(
                (report.exact_short, report.foreign_source, report.custody),
                (3, 1, 2)
            );
            assert!(report.measurement.work > 0 && report.measurement.peak > 0);
        },
    );
}
