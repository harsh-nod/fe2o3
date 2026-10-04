//! Execute genuine generated V50 obligations without promoting extraction custody.

use super::*;
use fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1 as Runtime;

const EXECUTION_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::execution_v50_tests::mixed_execution_child";
const RUNTIME_ROOT: &str = "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

#[derive(Debug, Serialize, Deserialize)]
struct ExecutionObservation {
    roots: usize,
    generated_bytes: usize,
    signed_receipt_bytes: usize,
    original: [u8; 32],
    output: [u8; 32],
    statement: [u8; 32],
    runtime: [u8; 32],
    extraction_refused: bool,
}

fn execute_original_candidate(
    candidate: PreparedMixedPublicationV28<'_, '_, '_>,
    runtime: &Runtime,
    budget: &mut Budget<'_>,
) -> Result<ExecutionObservation, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    candidate.replay(budget)?;
    let subject = candidate.refinement_subject(budget)?;
    let roots = candidate.worker(budget)?.root_count(budget)?;
    let generated_bytes = candidate.generated_source(budget)?.len();
    assert_eq!(roots, 2);
    assert!(generated_bytes > 0);
    assert!(!candidate.grants_publication_or_artifact_authority());
    let pending = candidate.prepare_execution_for_test(budget, 120)?;
    assert!(!pending.authenticates_executed_proof());
    let executed = match pending.execute(runtime, budget) {
        Ok(executed) => executed,
        Err(error) => {
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            return Err(Error::MixedRelocationExpressions(error));
        }
    };
    // Settle the real executed owner's storage even if receipt checks unwind.
    let observed = catch_unwind(AssertUnwindSafe(|| {
        executed
            .replay_signed_receipt(budget)
            .map_err(Error::MixedRelocationExpressions)?;
        assert_eq!(
            executed
                .subject(budget)
                .map_err(Error::MixedRelocationExpressions)?,
            subject,
        );
        assert_eq!(
            <[u8; 32]>::from(sha2::Sha256::digest(
                executed
                    .prefix_witness(budget)
                    .map_err(Error::MixedRelocationExpressions)?,
            )),
            subject.prefix_execution_identity(),
        );
        assert!(!executed.grants_publication_or_launch_authority());
        assert!(!executed.proves_mir_to_native_lowering());
        let signed_receipt_bytes = executed
            .signed_receipt(budget)
            .map_err(Error::MixedRelocationExpressions)?
            .len();
        assert!(signed_receipt_bytes > 0);
        Ok::<_, Error>(signed_receipt_bytes)
    }));
    let settled = executed
        .discard(budget)
        .map_err(Error::MixedRelocationExpressions);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let signed_receipt_bytes = match observed {
        Ok(Ok(bytes)) => {
            settled?;
            bytes
        }
        Ok(Err(error)) => return Err(error),
        Err(payload) => std::panic::resume_unwind(payload),
    };
    candidate.replay(budget)?;
    assert!(!candidate.grants_publication_or_artifact_authority());
    assert!(matches!(
        candidate.into_protected(budget),
        Err(Error::MixedPublication(
            MixedPublicationErrorV28::ExtractionOnly
        ))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    Ok(ExecutionObservation {
        roots,
        generated_bytes,
        signed_receipt_bytes,
        original: *subject.graph_identities()[0].digest(),
        output: *subject.graph_identities()[3].digest(),
        statement: subject.statement_identity(),
        runtime: runtime.identity().as_bytes(),
        extraction_refused: true,
    })
}

struct ExecutionCallbacks {
    runtime: Runtime,
    result: Option<Result<ExecutionObservation, String>>,
}

impl Callbacks for ExecutionCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            transaction
                .with_original_source_mixed_publication_test_limits_v28(
                    500_000_000,
                    20_000_000,
                    |candidate, budget| {
                        execute_original_candidate(candidate, &self.runtime, budget)
                    },
                )
                .map(|result| result.into_observation())
                .map_err(|error| format!("actual extraction-owned V50 execution: {error:?}"))
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "private actual-rustc runtime child; invoked only by the parent fixture"]
fn mixed_execution_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let runtime =
        Runtime::open_pinned_contexts_v3(RUNTIME_ROOT).expect("admitted pinned runtime required");
    let mut callbacks = ExecutionCallbacks {
        runtime,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual V50 execution callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("V50 execution result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual extraction-owned V50 execution: {result:?}"
    );
}

#[test]
#[ignore = "requires admitted pinned Verus runtime, nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mixed_composition_executes_without_protected_custody() {
    let runtime =
        Runtime::open_pinned_contexts_v3(RUNTIME_ROOT).expect("admitted pinned runtime required");
    let expected_runtime = runtime.identity().as_bytes();
    runtime.revalidate().expect("runtime preflight");
    drop(runtime);
    run_actual_sources::<ExecutionObservation>(
        &[("straight", STRAIGHT)],
        &[(0, 0)],
        EXECUTION_CHILD,
        "ORIGINAL_SOURCE_MIXED_EXECUTION_V50_EXTRACTION_ONLY",
        program,
        |_, _, _, report, _| {
            assert_eq!(report.roots, 2);
            assert!(report.generated_bytes > 0 && report.signed_receipt_bytes > 0);
            assert_ne!(report.original, [0; 32]);
            assert_ne!(report.output, [0; 32]);
            assert_ne!(report.statement, [0; 32]);
            assert_eq!(report.runtime, expected_runtime);
            assert!(report.extraction_refused);
        },
    );
}
