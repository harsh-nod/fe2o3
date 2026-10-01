//! Genuine rustc capture through the existing Worker FinalConsumer. No runtime
//! is fabricated or invoked, and generated source is not an executed receipt.
use super::*;

const ORIGINAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::original_mir_v30_tests::original_mir_worker_child";
const SCALAR: &str = "let temporary = a ^ b; let _result = temporary | a;";
const CONTROL: &str = "let _result = if a == b { a ^ b } else { a | b };";

fn original_program(body: &str) -> String {
    let kernel = |name| {
        format!(
            r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(a: u32, b: u32) {{ {body} }}
"#
        )
    };
    format!(
        "use fe2o3_device::kernel;\n{}{}",
        kernel("z_original"),
        kernel("a_original")
    )
}

#[derive(Debug, Serialize, Deserialize)]
struct OriginalObservation {
    census: [usize; 6],
    statement: [u8; 32],
    work: usize,
    peak: usize,
}

fn original_observe(
    candidate: PreparedMixedPublicationV28<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<OriginalObservation, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    candidate.replay(budget)?;
    let request = candidate.original_mir_request(budget)?;
    request
        .check_original_source(candidate.source(budget)?, budget)
        .map_err(Error::OriginalMir)?;
    let subject = request.subject(budget).map_err(Error::OriginalMir)?;
    let source = std::str::from_utf8(
        request
            .generated_source(budget)
            .map_err(Error::OriginalMir)?,
    )
    .unwrap();
    assert!(source.contains("original_mir_cfg_refines_canonical_0_v31"));
    assert!(source.contains("original_mir_cfg_refines_canonical_1_v31"));
    assert!(source.contains("original_control_step_relation_0_v31"));
    assert!(!source.contains("assume("));
    assert_eq!(&subject.census()[..2], &[2, 2]);
    assert!(subject.census()[3] >= 4);
    let optimized = candidate.refinement_subject(budget)?.expressions();
    assert_eq!(subject.canonical_identity(), optimized.input());
    assert_eq!(
        subject.semantic_identity(),
        optimized.source_semantic_identity()
    );
    assert_eq!(subject.ssa_identity(), optimized.source_ssa_identity());
    assert_eq!(candidate.worker(budget)?.root_count(budget)?, 2);
    assert!(!candidate.worker(budget)?.llvm_ir(budget)?.is_empty());
    assert!(!request.authenticates_executed_proof());
    assert!(!request.grants_artifact_or_launch_authority());
    assert!(
        candidate
            .open_gates()
            .contains(&MixedPublicationOpenGateV28::OriginalMirToKirRefinement)
    );
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    Ok(OriginalObservation {
        census: subject.census(),
        statement: subject.statement_identity(),
        work: budget.work(),
        peak: budget.peak_storage(),
    })
}

#[derive(Default)]
struct OriginalCallbacks {
    result: Option<Result<OriginalObservation, String>>,
}

// The on-account API retains root-phase charges until its enclosing transaction
// ends. Call this only after its continuation and all original bindings are gone.
fn finish_original_root_phase(budget: &mut Budget<'_>) {
    let retained = budget.storage().checked_sub(41).unwrap();
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 41);
}

impl Callbacks for OriginalCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(41).unwrap();
                let mut entered = 0;
                let result = transaction()?.with_original_source_mixed_publication_on_account_v28(
                    &mut budget,
                    |candidate, budget| {
                        entered += 1;
                        original_observe(candidate, budget)
                    },
                );
                let result = result.map(|result| result.into_observation());
                finish_original_root_phase(&mut budget);
                Ok::<_, String>((result, entered, budget.work(), budget.peak_storage()))
            };
            let (measured, entered, work, peak) = run(500_000_000, 64_000_000)?;
            let mut measured = match measured {
                Ok(value) => {
                    assert_eq!(entered, 1);
                    value
                }
                Err(error) => return Err(format!("original MIR Worker request: {error:?}")),
            };
            let (exact, entered, exact_work, exact_peak) = run(work, peak)?;
            let exact =
                exact.map_err(|error| format!("exact original MIR Worker request: {error:?}"))?;
            assert_eq!(entered, 1);
            assert_eq!(exact.statement, measured.statement);
            assert_eq!((exact_work, exact_peak), (work, peak));
            for short_work in [true, false] {
                let (short, _, _, _) = run(
                    work - usize::from(short_work),
                    peak - usize::from(!short_work),
                )?;
                let error = match short {
                    Err(error) => error,
                    Ok(_) => panic!("one-short original MIR Worker account admitted"),
                };
                if short_work {
                    let limit = work_refusal(&error);
                    assert_eq!((limit.actual(), limit.limit()), (work, work - 1));
                } else {
                    let limit = storage_refusal(&error);
                    assert_eq!((limit.actual(), limit.limit()), (peak, peak - 1));
                }
            }
            let mut foreign_calls = 0;
            let foreign = transaction()?.with_original_source_mixed_publication_v28::<(), _>(
                |candidate, original| {
                    foreign_calls += 1;
                    let request = candidate.original_mir_request(original)?;
                    let mut work = Work::new(500_000_000);
                    let mut foreign = Budget::new(&mut work, 64_000_000);
                    foreign.reserve_storage(original.storage())?;
                    let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                    let error = request.generated_source(&foreign).unwrap_err();
                    assert_eq!(
                        (foreign.work(), foreign.storage(), foreign.peak_storage()),
                        before
                    );
                    assert!(matches!(
                        error,
                        fe2o3_verifier::MixedOptimizerRefinementErrorV26::Source(
                            ProductionSourceOwnedViewErrorV18::Resource(Resource::Accounting)
                        )
                    ));
                    Err(Error::OriginalMir(error))
                },
            );
            assert_eq!(foreign_calls, 1);
            assert!(matches!(
                foreign,
                Err(Error::OriginalMir(
                    fe2o3_verifier::MixedOptimizerRefinementErrorV26::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(Resource::Accounting)
                    )
                ))
            ));
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 64_000_000);
            budget.reserve_storage(41).unwrap();
            let selected = transaction()?
                .with_original_source_mixed_publication_on_account_v28::<(), _>(
                    &mut budget,
                    |_, _| Err(Error::Unsupported("original MIR consumer refusal")),
                );
            assert!(matches!(
                selected,
                Err(Error::Unsupported("original MIR consumer refusal"))
            ));
            drop(selected);
            finish_original_root_phase(&mut budget);
            let pending = transaction()?;
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                pending.with_original_source_mixed_publication_on_account_v28::<(), _>(
                    &mut budget,
                    |_, _| std::panic::panic_any(930u32),
                )
            }));
            let payload = match unwind {
                Err(payload) => payload,
                Ok(_) => panic!("original MIR consumer unwind was swallowed"),
            };
            assert_eq!(*payload.downcast::<u32>().unwrap(), 930);
            finish_original_root_phase(&mut budget);
            measured.work = exact_work;
            measured.peak = peak;
            Ok(measured)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "private actual-rustc child; invoked only by the parent fixture"]
fn original_mir_worker_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = OriginalCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual original MIR callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("original MIR result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual original MIR Worker candidate: {result:?}"
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mir_cfg_request_reaches_worker_with_scalar_and_control() {
    run_actual_sources::<OriginalObservation>(
        &[("scalar", SCALAR), ("control", CONTROL)],
        &[(0, 0)],
        ORIGINAL_CHILD,
        "ORIGINAL_MIR_WORKER_V31",
        original_program,
        |_, _, _, report, _| {
            assert!(report.work > 0 && report.peak > 0);
            assert_eq!(&report.census[..2], &[2, 2]);
            assert_ne!(report.statement, [0; 32]);
        },
    );
}
