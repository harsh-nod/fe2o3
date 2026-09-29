//! Actual rustc capture through the distinct source-bound Policy10 continuation.
use super::*;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_pure_cse_tests::mixed_pure_cse_child";
const DUPLICATE: &str = r#"
    if i >= input.len() { return; }
    let value = input[i];
    let first = value ^ seed;
    let second = value ^ seed;
    *slot = first | second;
"#;
const STORE: &str = "*slot = seed;";
const NESTED: &str = r#"
    if i >= input.len() { return; }
    *slot = read_outer(&input[i]);
"#;

fn program(body: &str) -> String {
    format!(
        r#"use fe2o3_device::{{DisjointSlice, kernel, thread}};
#[inline(never)]
fn read_leaf(value: &u32) -> u32 {{ *value }}
#[inline(never)]
fn read_outer(value: &u32) -> u32 {{ read_leaf(value) }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn mixed(input: &[u32], mut output: DisjointSlice<u32>, seed: u32) {{
    let index = thread::index_1d();
    let i = index.get();
    let Some(slot) = output.get_mut(index) else {{ return; }};
    {body}
}}
"#
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    policy: u16,
    historical_policy: u16,
    cse_changed: bool,
    binary_before: usize,
    binary_after: usize,
    reads: usize,
    writes: usize,
    unused_slices: usize,
    foreign_owner_refused: bool,
    incomplete_abi_refused: bool,
    callback_error_preserved: bool,
}

#[derive(Default)]
struct PureCseCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for PureCseCallbacks {
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
                .map_err(|error| format!("independent Policy10 source: {error:?}"))?;
            let continuation = transaction()?
                .with_original_source_conditional_mixed_pure_cse_v26(
                    |source, handoff, roots, _, budget| {
                        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                        handoff.check_original_argument_abi_v26(AbiInput { roots }, budget)?;
                        let original = source.canonical(budget)?;
                        let output = handoff.output(budget)?;
                        assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                        assert_eq!(
                            source.source_semantic(budget)?.wire_version(),
                            SemanticMirWireVersionV1::V35
                        );
                        assert!(!handoff.runtime_requirements_are_discharged());
                        assert!(!handoff.ranked_verification_is_complete());
                        assert!(!handoff.grants_artifact_or_launch_authority());
                        use fe2o3_pliron::PlironOptimizationPassV1 as Pass;
                        assert_eq!(
                            output
                                .report()
                                .passes()
                                .iter()
                                .map(|row| row.pass())
                                .collect::<Vec<_>>(),
                            [
                                Pass::IntegerNeutralWorklistCanonicalization,
                                Pass::LocalPureCommonSubexpressionElimination,
                                Pass::DominancePureCommonSubexpressionElimination,
                                Pass::DeadCodeElimination,
                            ]
                        );
                        let binaries = |module: &fe2o3_kernel_ir::Module| {
                            module
                                .functions
                                .iter()
                                .filter_map(|function| function.body.as_ref())
                                .flat_map(|body| &body.blocks)
                                .flat_map(|block| &block.operations)
                                .filter(|operation| {
                                    matches!(
                                        operation.kind,
                                        fe2o3_kernel_ir::OperationKind::Binary { .. }
                                    )
                                })
                                .count()
                        };
                        let premises = handoff.runtime_premises(budget)?;
                        assert_eq!(premises.len(), 2);
                        let occurrences = handoff.runtime_occurrences(budget)?;
                        let reads = occurrences
                            .iter()
                            .filter(|row| !row.domain().writing())
                            .count();
                        let writes = occurrences
                            .iter()
                            .filter(|row| row.domain().writing())
                            .count();
                        assert_eq!(
                            premises
                                .iter()
                                .map(|row| row.access_counts()[0])
                                .sum::<usize>(),
                            reads
                        );
                        assert_eq!(
                            premises
                                .iter()
                                .map(|row| row.access_counts()[1])
                                .sum::<usize>(),
                            writes
                        );
                        Ok(Observation {
                            policy: output.execution().policy_version(),
                            historical_policy: 0,
                            cse_changed: output.report().passes()[1].changed()
                                || output.report().passes()[2].changed(),
                            binary_before: binaries(original.module()),
                            binary_after: binaries(output.owner().module()),
                            reads,
                            writes,
                            unused_slices: premises
                                .iter()
                                .filter(|row| row.access_counts() == [0, 0])
                                .count(),
                            foreign_owner_refused: false,
                            incomplete_abi_refused: false,
                            callback_error_preserved: false,
                        })
                    },
                )
                .map_err(|error| format!("Policy10 continuation: {error:?}"))?;
            let mut observation = continuation.into_observation();
            observation.historical_policy = transaction()?
                .with_original_source_conditional_mixed_worklist_v26(|_, handoff, _, _, budget| {
                    Ok(handoff.output(budget)?.execution().policy_version())
                })
                .map_err(|error| format!("historical Policy9: {error:?}"))?
                .into_observation();
            let mut foreign_called = false;
            let foreign_result = transaction()?
                .with_original_source_conditional_mixed_pure_cse_v26::<(), _>(
                    |_, handoff, _, _, budget| {
                        foreign_called = true;
                        handoff.check_original_source(&foreign, budget)?;
                        Ok(())
                    },
                );
            assert!(foreign_called);
            assert!(matches!(
                foreign_result,
                Err(Error::Source(SourceError::Binding(_)))
            ));
            observation.foreign_owner_refused = true;
            let mut abi_called = false;
            let abi_result = transaction()?
                .with_original_source_conditional_mixed_pure_cse_v26::<(), _>(
                    |_, handoff, _, _, budget| {
                        abi_called = true;
                        handoff.check_original_argument_abi_v26(AbiInput { roots: &[] }, budget)?;
                        Ok(())
                    },
                );
            assert!(abi_called);
            assert!(matches!(
                abi_result,
                Err(Error::Source(SourceError::Binding(
                    "kernel argument ABI profile differs from its original descriptor/source contract"
                )))
            ));
            observation.incomplete_abi_refused = true;
            let callback_result = transaction()?
                .with_original_source_conditional_mixed_pure_cse_v26::<(), _>(|_, _, _, _, _| {
                    Err(Error::Unsupported("Policy10 consumer refusal"))
                });
            observation.callback_error_preserved = matches!(
                callback_result,
                Err(Error::Unsupported("Policy10 consumer refusal"))
            );
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
fn mixed_pure_cse_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PureCseCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("Policy10 callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("Policy10 result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual mixed Policy10: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic AMD dependencies"]
fn actual_original_mixed_pure_cse_preserves_memory_contracts_and_observes_real_rewrites() {
    run_actual_sources::<Observation>(
        &[
            ("duplicate", DUPLICATE),
            ("store", STORE),
            ("nested", NESTED),
        ],
        &[(0, 0)],
        CHILD,
        "CONDITIONAL_MIXED_PURE_CSE_V26",
        program,
        |_, _, case, report, _| {
            assert_eq!((report.policy, report.historical_policy), (10, 9));
            assert_eq!(report.writes, 1);
            assert_eq!(report.reads, usize::from(case != "store"));
            assert_eq!(report.unused_slices, usize::from(case == "store"));
            if case == "duplicate" {
                assert!(report.cse_changed);
                assert!(report.binary_after < report.binary_before);
            }
            assert!(report.foreign_owner_refused && report.incomplete_abi_refused);
            assert!(report.callback_error_preserved);
        },
    );
}
