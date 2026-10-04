//! Real registered Rust + independent CPU body + protected V50 execution.
//! Extraction custody remains inert even after a genuine successful proof.
use super::*;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::execution_v50_tests::reference_execution_v69_tests::registered_reference_execution_child";
const NEGATIVE_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::execution_v50_tests::reference_execution_v69_tests::registered_reference_refusal_child";

#[derive(Debug, Serialize, Deserialize)]
struct Observation {
    proved: bool,
    semantic_refusal: bool,
    generated_bytes: usize,
    roots: usize,
    extraction_refused: bool,
}

struct ReferenceExecutionCallbacks {
    runtime: Runtime,
    negative: bool,
    result: Option<Result<Observation, String>>,
}

impl Callbacks for ReferenceExecutionCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            transaction
                .with_original_source_mixed_publication_test_limits_v28(
                    500_000_000,
                    64_000_000,
                    |candidate, budget| {
                        let text =
                            std::str::from_utf8(candidate.generated_source(budget)?).unwrap();
                        for root in 0..2 {
                            for name in [
                                "complete_source",
                                "complete_final",
                                "computations",
                                "active_count",
                            ] {
                                assert!(
                                    text.contains(&format!(
                                        "invocation_reference_{name}_{root}_v69("
                                    ))
                                );
                            }
                        }
                        assert_eq!(text.matches("// registered-reference-v69 root=").count(), 2);
                        let generated_bytes = text.len();
                        if !self.negative {
                            let proved =
                                execute_original_candidate(candidate, &self.runtime, budget)?;
                            assert!(proved.signed_receipt_bytes > 0);
                            return Ok(Observation {
                                proved: true,
                                semantic_refusal: false,
                                generated_bytes,
                                roots: proved.roots,
                                extraction_refused: proved.extraction_refused,
                            });
                        }
                        let floor = budget.storage();
                        let ledger = budget.work_ledger_identity_v1();
                        let pending = candidate.prepare_execution_for_test(budget, 120)?;
                        let error = match pending.execute(&self.runtime, budget) {
                            Ok(executed) => {
                                executed
                                    .discard(budget)
                                    .map_err(Error::MixedRelocationExpressions)?;
                                panic!("incorrect registered source unexpectedly proved");
                            }
                            Err(error) => error,
                        };
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work_ledger_identity_v1() == ledger);
                        // Frontend, resource, ownership and runtime setup failures
                        // cannot masquerade as rejection of the semantic mutant.
                        let mut cause: &(dyn std::error::Error + 'static) = &error;
                        let mut semantic = false;
                        loop {
                            let detail = cause.to_string();
                            semantic |= detail.contains("postcondition not satisfied")
                                && detail.contains("invocation_reference_complete_source_");
                            match cause.source() {
                                Some(next) => cause = next,
                                None => break,
                            }
                        }
                        assert!(
                            semantic,
                            "not an observed reference postcondition failure: {error:?}"
                        );
                        assert!(!candidate.grants_publication_or_artifact_authority());
                        Ok(Observation {
                            proved: false,
                            semantic_refusal: true,
                            generated_bytes,
                            roots: 2,
                            extraction_refused: true,
                        })
                    },
                )
                .map(|value| value.into_observation())
                .map_err(|error| format!("registered CPU/source execution: {error:?}"))
        })());
        Compilation::Stop
    }
}

fn child(negative: bool) {
    let args: Vec<String> = serde_json::from_slice(
        &std::fs::read(env::var_os(ARGS).expect("reference proof child arguments")).unwrap(),
    )
    .unwrap();
    let runtime = Runtime::open_pinned_contexts_v3(RUNTIME_ROOT)
        .expect("genuine protected runtime required for reference equations");
    let mut callbacks = ReferenceExecutionCallbacks {
        runtime,
        negative,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual registered reference callback not entered");
    std::fs::write(
        env::var_os(RESULT).expect("reference proof result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "registered source proof child: {result:?}");
}

#[test]
#[ignore = "private rustc/protected proof child; only the owning parent may invoke it"]
fn registered_reference_execution_child() {
    child(false);
}

#[test]
#[ignore = "private semantic-mutant child; only the owning parent may invoke it"]
fn registered_reference_refusal_child() {
    child(true);
}

fn program(case: &str) -> String {
    if matches!(case, "multiple_outputs" | "foreign_write") {
        let mut source = "use fe2o3_device::{DisjointSlice, kernel, thread};\n".to_owned();
        for (name, first, second) in [("alpha", 17, 19), ("zeta", 23, 29)] {
            let selected = if case == "foreign_write" {
                "right"
            } else {
                "left"
            };
            source.push_str(&format!(
                r#"
fn {name}_reference(_point: usize, left: &mut u32, right: &mut u32) {{
    *left = {first}; *right = {second};
}}
#[kernel(typed, reference = {name}_reference, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn {name}(mut left: DisjointSlice<u32>, mut right: DisjointSlice<u32>) {{
    if let Some(element) = {selected}.get_mut(thread::index_1d()) {{ *element = {first}; }}
    if let Some(element) = right.get_mut(thread::index_1d()) {{ *element = {second}; }}
}}
"#
            ));
        }
        return source;
    }
    let statement = match case {
        "fill" => "*element = VALUE;",
        "wrong_value" => "*element = VALUE ^ 1;",
        "missing" => "let _ = element;",
        "duplicate" => "*element = VALUE; *element = VALUE;",
        "guard_skipped" => "if length > 1 { *element = VALUE; }",
        _ => panic!("unknown source mutant"),
    };
    let mut source = "use fe2o3_device::{DisjointSlice, kernel, thread};\n".to_owned();
    for (name, value) in [("alpha", 17), ("zeta", 23)] {
        source.push_str(&format!(
            r#"
fn {name}_reference(_point: usize, output: &mut u32) {{ *output = {value}; }}
#[kernel(typed, reference = {name}_reference, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn {name}(mut output: DisjointSlice<u32>) {{
    let length = output.len();
    if let Some(element) = output.get_mut(thread::index_1d()) {{ {} }}
}}
"#,
            statement.replace("VALUE", &value.to_string())
        ));
    }
    source
}

#[test]
#[ignore = "requires actual rustc imports and the admitted pinned Verus runtime"]
fn actual_registered_scalar_reference_proves_in_range_and_empty_domain_completion() {
    Runtime::open_pinned_contexts_v3(RUNTIME_ROOT)
        .unwrap()
        .revalidate()
        .unwrap();
    run_actual_sources::<Observation>(
        &[("fill", "fill"), ("multiple_outputs", "multiple_outputs")],
        &[(0, 0)],
        CHILD,
        "REGISTERED_SOURCE_REFERENCE_EXECUTION_V69",
        program,
        |_, _, _, report, _| {
            assert!(report.proved && !report.semantic_refusal && report.extraction_refused);
            assert_eq!(report.roots, 2);
            assert!(report.generated_bytes > 0);
        },
    );
}

#[test]
#[ignore = "requires actual semantic-mutant source and the admitted pinned Verus runtime"]
fn actual_registered_scalar_reference_rejects_value_coverage_and_guard_mutants() {
    Runtime::open_pinned_contexts_v3(RUNTIME_ROOT)
        .unwrap()
        .revalidate()
        .unwrap();
    run_actual_sources::<Observation>(
        &[
            ("wrong_value", "wrong_value"),
            ("missing", "missing"),
            ("duplicate", "duplicate"),
            ("guard_skipped", "guard_skipped"),
            ("foreign_write", "foreign_write"),
        ],
        &[(0, 0)],
        NEGATIVE_CHILD,
        "REGISTERED_SOURCE_REFERENCE_REFUSAL_V69",
        program,
        |_, _, _, report, _| {
            assert!(!report.proved && report.semantic_refusal && report.extraction_refused);
            assert_eq!(report.roots, 2);
            assert!(report.generated_bytes > 0);
        },
    );
}

#[test]
#[ignore = "requires actual rustc imports; exercises existing exact-budget/ledger/unwind checks, not proof execution"]
fn actual_registered_scalar_reference_retains_exact_budget_and_custody_boundaries() {
    const ORIGINAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::original_mir_v30_tests::original_mir_worker_child";
    run_actual_sources::<serde_json::Value>(
        &[("fill", "fill")],
        &[(0, 0)],
        ORIGINAL_CHILD,
        "REGISTERED_REFERENCE_EXISTING_CUSTODY_V69",
        program,
        |_, _, _, report, _| {
            assert_eq!(report["census"][0].as_u64(), Some(2));
            assert!(report["work"].as_u64().unwrap() > 0);
            assert!(report["peak"].as_u64().unwrap() > 41);
        },
    );
}

#[test]
#[ignore = "requires actual two-target rustc imports and original resource/custody checks; no proof execution"]
fn actual_registered_scalar_reference_length_calls_retain_cumulative_custody() {
    const ORIGINAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::original_mir_v30_tests::original_mir_worker_child";
    run_actual_sources::<serde_json::Value>(
        &[
            ("single_length", "single_length"),
            ("repeated_lengths", "repeated_lengths"),
        ],
        &[(0, 0)],
        ORIGINAL_CHILD,
        "REGISTERED_REFERENCE_LENGTH_CUSTODY_V76",
        |case| {
            let source = program("fill");
            match case {
                "single_length" => source,
                "repeated_lengths" => source.replace(
                    "let length = output.len();",
                    "let length = output.len(); let second_length = output.len();",
                ),
                _ => panic!("unknown length source case"),
            }
        },
        |_, _, _, report, _| {
            assert_eq!(report["census"][0].as_u64(), Some(2));
            assert!(report["work"].as_u64().unwrap() > 0);
            assert!(report["peak"].as_u64().unwrap() > 41);
        },
    );
}

#[test]
#[ignore = "requires actual two-target WriteOnlyDisjointSlice imports; source custody only, no reference proof execution"]
fn actual_write_only_source_length_calls_retain_cumulative_custody() {
    const ORIGINAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::original_mir_v30_tests::original_mir_worker_child";
    run_actual_sources::<serde_json::Value>(
        &[("write_only_lengths", "write_only_lengths")],
        &[(0, 0)],
        ORIGINAL_CHILD,
        "WRITE_ONLY_SOURCE_LENGTH_CUSTODY_V76",
        |_| {
            let mut source =
                "use fe2o3_device::{WriteOnlyDisjointSlice, kernel, thread};\n".to_owned();
            for (name, value) in [("alpha", 17), ("zeta", 23)] {
                source.push_str(&format!(
                    r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn {name}(mut output: WriteOnlyDisjointSlice<u32>) {{
    let length = output.len();
    let second_length = output.len();
    let _ = output.write(thread::index_1d(), {value});
}}
"#
                ));
            }
            source
        },
        |_, _, _, report, _| {
            assert_eq!(report["census"][0].as_u64(), Some(2));
            assert!(report["work"].as_u64().unwrap() > 0);
            assert!(report["peak"].as_u64().unwrap() > 41);
        },
    );
}
