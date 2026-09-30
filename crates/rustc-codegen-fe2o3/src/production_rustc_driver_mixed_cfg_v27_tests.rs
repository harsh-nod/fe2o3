//! Genuine Rust composition controls. Generation and admitted execution are
//! separate parents; generation never reports that a theorem was executed.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

#[path = "production_rustc_driver_mixed_cfg_request_v27_tests.rs"]
mod request_tests;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_cfg_tests::mixed_cfg_child";
const EXECUTED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_cfg_tests::mixed_cfg_executed_child";
const DUPLICATE: &str =
    "let value = input[i]; let a = value ^ seed; let b = value ^ seed; *slot = a | b;";
const SHARED: &str = "let value = input[i]; *slot = value;";
const RMW: &str = "let old = *slot; *slot = old ^ seed;";

fn program(body: &str) -> String {
    format!(
        r#"use fe2o3_device::{{DisjointSlice,kernel,thread}};
#[kernel(typed,launch(required=[64,1,1],max=[64,1,1],max_grid=[3,1,1]))]
pub fn mixed(input:&[u32],mut output:DisjointSlice<u32>,seed:u32) {{
 let index=thread::index_1d();let i=index.get();
 if i>=input.len() {{return;}}
 let Some(slot)=output.get_mut(index) else {{return;}};
 {body}
}}
"#
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    policies: Vec<u16>,
    functions: Vec<usize>,
    blocks: Vec<usize>,
    source_bound: bool,
    conditional: bool,
    generated_cfg: bool,
    distinct_statements: bool,
    executed: bool,
}
#[derive(Default)]
struct CfgCallbacks {
    execute: bool,
    result: Option<Result<Observation, String>>,
}
impl Callbacks for CfgCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let mut observation = Observation {
                policies: vec![],
                functions: vec![],
                blocks: vec![],
                source_bound: true,
                conditional: true,
                generated_cfg: true,
                distinct_statements: false,
                executed: false,
            };
            let mut subjects = Vec::new();
            macro_rules! inspect {
                ($route:ident,$prepare:ident) => {{
                    let record = transaction()?
                        .$route(|source, handoff, roots, _, budget| {
                            handoff.check_original_argument_abi_v26(AbiInput { roots }, budget)?;
                            let floor = budget.storage();
                            let request = fe2o3_verifier::$prepare(source, handoff, budget)
                                .map_err(Error::ConditionalMixedCfg)?;
                            let inspected = (|| -> Result<_, Error> {
                                let subject = request
                                    .subject(budget)
                                    .map_err(Error::ConditionalMixedCfg)?;
                                let source_ssa = source.source_ssa(budget)?;
                                let bound = subject.source_ssa_identity()
                                    == *source_ssa.identity().as_bytes()
                                    && subject.source_semantic_identity()
                                        == *source_ssa.source_semantic_sha256()
                                    && subject.input() == *source.canonical(budget)?.identity()
                                    && subject.output()
                                        == *handoff.output(budget)?.owner().identity();
                                let bytes = request
                                    .generated_source(budget)
                                    .map_err(Error::ConditionalMixedCfg)?;
                                let text = std::str::from_utf8(bytes)
                                    .map_err(|_| Error::Unsupported("CFG generated UTF8"))?;
                                let cfg = text.contains("V27 exact total-operator congruence")
                                    && text.contains("cfg_function_trace_refinement_0_v26")
                                    && text.contains("cfg_related_v26(n, o, op)")
                                    && !text.contains("assume(")
                                    && !text.contains("external_body");
                                let conditional = !request.authenticates_executed_proof()
                                    && !request.proves_mir_to_native_lowering()
                                    && !request.grants_artifact_or_launch_authority()
                                    && !handoff.runtime_requirements_are_discharged()
                                    && !handoff.grants_artifact_or_launch_authority();
                                Ok((subject, bound, cfg, conditional))
                            })();
                            let settled = request.discard(budget);
                            let result = inspected?;
                            settled.map_err(Error::ConditionalMixedCfg)?;
                            if budget.storage() != floor {
                                return Err(Error::Resource(Resource::Accounting));
                            }
                            Ok(result)
                        })
                        .map_err(|error| format!("actual mixed CFG preparation: {error:?}"))?;
                    let (subject, bound, cfg, conditional) = record.into_observation();
                    observation.policies.push(subject.policy_version());
                    observation.functions.push(subject.modeled_functions());
                    observation.blocks.push(subject.modeled_blocks());
                    observation.source_bound &= bound;
                    observation.generated_cfg &= cfg;
                    observation.conditional &= conditional;
                    subjects.push(subject);
                }};
            }
            inspect!(
                with_original_source_conditional_mixed_worklist_v26,
                prepare_mixed_worklist_cfg_refinement_v27
            );
            inspect!(
                with_original_source_conditional_mixed_pure_cse_v26,
                prepare_mixed_pure_cse_cfg_refinement_v27
            );
            observation.distinct_statements =
                subjects[0].statement_identity() != subjects[1].statement_identity();
            if self.execute {
                let runtime = fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1::open(
                    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
                )
                .map_err(|error| format!("required admitted CFG proof runtime: {error:?}"))?;
                let worklist = transaction()?
                    .verify_original_source_mixed_worklist_cfg_v27(&runtime, 120)
                    .map_err(|error| format!("actual Policy9 CFG theorem: {error:?}"))?
                    .into_observation();
                let cse = transaction()?
                    .verify_original_source_mixed_pure_cse_cfg_v27(&runtime, 120)
                    .map_err(|error| format!("actual Policy10 CFG theorem: {error:?}"))?
                    .into_observation();
                // Fresh transactions can observe different optimizer witness
                // resources. Each executed request replays its own exact signed
                // statement; across requests only stable subject fields agree.
                let stable_subject = |subject: fe2o3_verifier::MixedOptimizerCfgSubjectV27| {
                    (
                        subject.source_semantic_identity(),
                        subject.source_ssa_identity(),
                        subject.input(),
                        subject.output(),
                        subject.policy_version(),
                        subject.modeled_functions(),
                        subject.modeled_blocks(),
                    )
                };
                if [worklist, cse]
                    .into_iter()
                    .zip(subjects.iter().copied())
                    .any(|(executed, prepared)| {
                        stable_subject(executed) != stable_subject(prepared)
                    })
                {
                    return Err(
                        "executed CFG subjects differ from genuine prepared subjects".into(),
                    );
                }
                observation.executed = true;
            }
            Ok(observation)
        })());
        Compilation::Stop
    }
}
fn child(execute: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = CfgCallbacks {
        execute,
        ..CfgCallbacks::default()
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual CFG callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("CFG result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual mixed CFG: {result:?}");
}
#[test]
#[ignore = "subprocess child: requires actual captured rustc arguments"]
fn mixed_cfg_child() {
    child(false)
}
#[test]
#[ignore = "subprocess child: requires captured rustc arguments and admitted Verus runtime"]
fn mixed_cfg_executed_child() {
    child(true)
}
fn check(report: Observation, execute: bool) {
    assert_eq!(report.policies, [9, 10]);
    assert!(report.functions.iter().all(|count| *count > 0));
    assert!(report.blocks.iter().all(|count| *count > 0));
    assert!(
        report.source_bound
            && report.conditional
            && report.generated_cfg
            && report.distinct_statements
    );
    assert_eq!(report.executed, execute);
}
#[test]
#[ignore = "requires pinned rust-src and authentic AMD dependencies; generates but does not execute proof"]
fn actual_original_mixed_cfg_requests_retain_both_nominal_source_graphs() {
    run_actual_sources::<Observation>(
        &[("duplicate", DUPLICATE), ("shared", SHARED), ("rmw", RMW)],
        &[(0, 0)],
        CHILD,
        "CONDITIONAL_MIXED_CFG_V27",
        program,
        |_, _, _, report, _| check(report, false),
    );
}
#[test]
#[ignore = "requires pinned rust-src, AMD dependencies and admitted protected Verus runtime; missing runtime fails"]
fn actual_original_mixed_cfg_stages_execute_exact_generated_theorems() {
    run_actual_sources::<Observation>(
        &[("duplicate", DUPLICATE), ("shared", SHARED), ("rmw", RMW)],
        &[(0, 0)],
        EXECUTED_CHILD,
        "EXECUTED_CONDITIONAL_MIXED_CFG_V27",
        program,
        |_, _, _, report, _| check(report, true),
    );
}
