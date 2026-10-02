//! Conditional source continuations are not default compilation authority.
use super::*;

const BOUNDARY_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_worklist_tests::boundary_tests::mixed_entry_boundary_child";
const SELECTED: &str = "selected mixed boundary consumer refusal";

#[derive(Clone, Copy)]
enum Control {
    Observe,
    Consumer,
    ForeignSource,
    IncompleteAbi,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct BoundaryObservation {
    policies: Vec<u16>,
    roots: Vec<usize>,
    callbacks: Vec<usize>,
    selected_refusals: usize,
    early_refusals: usize,
}

#[derive(Default)]
struct BoundaryCallbacks {
    result: Option<Result<BoundaryObservation, String>>,
}

impl Callbacks for BoundaryCallbacks {
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
                .map_err(|error| format!("foreign boundary source: {error:?}"))?;
            let mut report = BoundaryObservation {
                policies: vec![],
                roots: vec![],
                callbacks: vec![],
                selected_refusals: 0,
                early_refusals: 0,
            };
            macro_rules! controls {
                ($route:ident, $limited:ident, $policy:literal) => {{
                    for mode in [Control::Observe, Control::Consumer, Control::ForeignSource, Control::IncompleteAbi] {
                        let calls = Cell::new(0usize);
                        let result = transaction()?.$route(|source, handoff, roots, _, budget| {
                            calls.set(calls.get() + 1);
                            handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                            handoff.check_original_argument_abi_v26(AbiInput { roots }, budget)?;
                            let original = source.canonical(budget)?;
                            let output = handoff.output(budget)?;
                            assert_eq!(output.execution().policy_version(), $policy);
                            assert_eq!(output.execution().graph_schema(), 18);
                            assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                            assert_eq!(roots.len(), original.module().kernels.len());
                            assert_eq!(roots.len(), output.owner().module().kernels.len());
                            assert_eq!(source.source_semantic(budget)?.wire_version(), SemanticMirWireVersionV1::V35);
                            assert!(!handoff.runtime_requirements_are_discharged());
                            assert!(!handoff.ranked_verification_is_complete());
                            assert!(!handoff.grants_artifact_or_launch_authority());
                            match mode {
                                Control::Observe => Ok(roots.len()),
                                Control::Consumer => Err(Error::Unsupported(SELECTED)),
                                Control::ForeignSource => {
                                    handoff.check_original_source(&foreign, budget)?;
                                    panic!("foreign source must not enter a replacement policy")
                                }
                                Control::IncompleteAbi => {
                                    handoff.check_original_argument_abi_v26(AbiInput { roots: &[] }, budget)?;
                                    panic!("missing original ABI must not enter a replacement policy")
                                }
                            }
                        });
                        assert_eq!(calls.get(), 1, "selected nominal consumer runs exactly once");
                        report.callbacks.push(calls.get());
                        match mode {
                            Control::Observe => {
                                report.roots.push(result.map_err(|error| format!("policy {} boundary: {error:?}", $policy))?.into_observation());
                                report.policies.push($policy);
                            }
                            Control::Consumer => {
                                assert!(matches!(refused(result), Error::Unsupported(SELECTED)));
                                report.selected_refusals += 1;
                            }
                            Control::ForeignSource => {
                                assert!(matches!(refused(result), Error::Source(SourceError::Binding("foreign original SSA owner"))));
                                report.selected_refusals += 1;
                            }
                            Control::IncompleteAbi => {
                                assert!(matches!(refused(result), Error::Source(SourceError::Binding("kernel argument ABI profile differs from its original descriptor/source contract"))));
                                report.selected_refusals += 1;
                            }
                        }
                    }
                    let calls = Cell::new(0usize);
                    let refused = refused(transaction()?.$limited::<(), _>(0, 20_000_000, |_, _, _, _, _| {
                        calls.set(calls.get() + 1);
                        Ok(())
                    }));
                    assert_eq!(calls.get(), 0, "early admission refusal cannot enter any consumer");
                    assert!(matches!(refused, Error::Resource(ResourceError::Work(error)) if error.limit() == 0 && error.actual() > 0));
                    report.early_refusals += 1;
                }};
            }
            controls!(
                with_original_source_conditional_mixed_worklist_v26,
                with_original_source_conditional_mixed_test_limits_v26,
                9
            );
            controls!(
                with_original_source_conditional_mixed_pure_cse_v26,
                with_original_source_conditional_mixed_pure_cse_test_limits_v27,
                10
            );
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess child: requires authentic captured rustc arguments"]
fn mixed_entry_boundary_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = BoundaryCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("mixed boundary callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("mixed boundary result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "mixed entry boundary: {result:?}");
}

#[test]
#[ignore = "requires pinned rust-src and authentic AMD dependencies; no LLVM or Verus execution"]
fn actual_mixed_entries_preserve_nominal_custody_and_never_retry_selected_refusals() {
    run_actual_sources::<BoundaryObservation>(
        &[
            ("copy", COPY),
            ("disjoint-copy", DISJOINT_COPY),
            ("rmw", RMW),
            ("nested-mutable", MUTABLE_RMW),
            ("repeated", REPEATED),
        ],
        &[(0, 0)],
        BOUNDARY_CHILD,
        "MIXED_ENTRY_BOUNDARY_V27",
        |body| program_roots(&[("boundary_a", body), ("boundary_b", body)]),
        |_, _, _, report, _| {
            assert_eq!(report.policies, [9, 10]);
            assert_eq!(report.roots, [2, 2]);
            assert_eq!(report.callbacks, [1; 8]);
            assert_eq!(report.selected_refusals, 6);
            assert_eq!(report.early_refusals, 2);
        },
    );
}

#[test]
fn default_target_entry_keeps_conditional_mixed_continuations_outside_publication() {
    let pipeline = include_str!("production_pipeline.rs");
    let collected = pipeline
        .split_once("impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {")
        .unwrap()
        .1;
    let lower = collected
        .split_once("pub(crate) fn lower_production_target(")
        .unwrap()
        .1
        .split_once("pub(crate) fn export_simulation_bundle_v1(")
        .unwrap()
        .0;
    let mut remaining = lower;
    for stage in [
        "self.import_semantic_mir()?",
        ".construct_semantic_middle_end()?",
        ".construct_semantic_ssa()?",
        ".materialize_target_neutral()?",
        ".verify_general_kernel_checks()?",
        "return Err(ranked.conditional_production_finalizer_refusal_v5(target_budget));",
        ".attach_target_neutral_checks()?",
        ".admit_formal_memory()?",
        ".lower_production_target()",
    ] {
        assert_eq!(
            lower.matches(stage).count(),
            1,
            "exact default stage: {stage}"
        );
        remaining = remaining.split_once(stage).unwrap().1;
    }
    for alternate in [
        "conditional_mixed",
        "worker_input_v26",
        "unwrap_or",
        "or_else(",
        "or(",
    ] {
        assert!(
            !lower.contains(alternate),
            "default must not reinterpret a conditional continuation: {alternate}"
        );
    }
    let publish = collected
        .split_once("pub(crate) fn publish_worker_handoff(")
        .unwrap()
        .1
        .split_once("pub(crate) fn require_semantic_mir_import(")
        .unwrap()
        .0;
    assert!(
        publish
            .contains("self.publish_mixed_worker_handoff_v53(target_budget, compiler_execution)")
    );
    for alternate in [
        "lower_production_target(",
        "unwrap_or",
        "or_else(",
        "if ",
        "match ",
    ] {
        assert!(
            !publish.contains(alternate),
            "publication alternate: {alternate}"
        );
    }
    assert!(!publish.contains("conditional_mixed"));
    let typed = include_str!("production_pipeline_source_mixed_publish_v53.rs")
        .split_once("pub(crate) fn publish_mixed_worker_handoff_v53(")
        .unwrap()
        .1;
    let mut remaining = typed;
    for stage in [
        "FunctionalRefinementVerusRuntimeLeaseV1::open(",
        "self.with_original_source_mixed_publication_on_account_v28(",
        "prepared.into_protected(budget)?",
        "protected.with_executed_composition_v29(",
        "executed.with_strict_handoff_v53(",
        "executed.check_strict_handoff_v53(handoff, budget)?",
        "protected.revalidate(budget)?",
        "publish_compiler_module_handoff_v3(",
        "compiler_execution.acquire(",
        "publish_compiler_execution_receipt_transport_v1(",
    ] {
        remaining = remaining.split_once(stage).unwrap().1;
    }
    assert!(!typed.contains("unwrap_or") && !typed.contains("or_else("));
    assert!(!typed.contains("lower_production_target("));
    for route in [
        include_str!("production_pipeline_source_mixed_worklist_v26.rs"),
        include_str!("production_pipeline_source_mixed_pure_cse_v26.rs"),
    ] {
        assert!(route.contains("ImportProfile::NominalV35"));
        assert!(!route.contains(".or_else("));
        assert!(!route.contains(".unwrap_or"));
    }
}
