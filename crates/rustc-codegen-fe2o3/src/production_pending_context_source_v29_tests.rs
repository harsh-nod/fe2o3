//! Actual Rust through the shared prepared-source path; this is not execution admission.
use super::*;
use crate::production_pipeline::ProductionPipelineError as PipelineError;
use fe2o3_kernel_ir::{AddressSpace, Constant, ExecutionOperationV15 as Execution, OperationKind};
use fe2o3_lower_mir_kernel::ProductionPendingScopedSourceOwnerV29 as Pending;

const PENDING_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::pending_context_source_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct PendingObservation {
    source: [u8; 32],
    graph: [u8; 32],
    canonical_bytes: u64,
    kernels: usize,
    context_issues: usize,
    derives: usize,
    scope_ends: usize,
    global_stores: usize,
    literals: [u32; 32],
    literal_count: usize,
    assertion_attachments: usize,
    replay_storage_stable: bool,
}

fn inspect(owner: &Pending, budget: &mut Budget<'_>) -> Result<PendingObservation, PipelineError> {
    let resource = |error: ResourceError| PipelineError::ContextHandoff(error.into());
    let identity = *owner.pending_identity();
    let storage = budget.storage();
    owner
        .replay_with_budget(budget)
        .map_err(PipelineError::PendingScopedSource)?;
    assert_eq!(*owner.pending_identity(), identity);
    assert_eq!(budget.storage(), storage);
    budget
        .reserve_storage(std::mem::size_of::<PendingObservation>())
        .map_err(resource)?;
    let mut result = PendingObservation {
        source: *owner.source_semantic_sha256(),
        graph: *identity.digest(),
        canonical_bytes: identity.canonical_length(),
        kernels: owner.pending_module().kernels.len(),
        context_issues: 0,
        derives: 0,
        scope_ends: 0,
        global_stores: 0,
        literals: [0; 32],
        literal_count: 0,
        assertion_attachments: owner.assertion_attachment_count(),
        replay_storage_stable: budget.storage()
            == storage + std::mem::size_of::<PendingObservation>(),
    };
    for function in &owner.pending_module().functions {
        budget.charge_work(1).map_err(resource)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1).map_err(resource)?;
                for operation in &block.operations {
                    budget.charge_work(33).map_err(resource)?;
                    match &operation.kind {
                        OperationKind::Execution(Execution::ContextIssue) => {
                            result.context_issues += 1
                        }
                        OperationKind::Execution(Execution::WorkgroupDerive { .. }) => {
                            result.derives += 1
                        }
                        OperationKind::Execution(Execution::ScopeEnd { .. }) => {
                            result.scope_ends += 1
                        }
                        OperationKind::Store { access, .. }
                        | OperationKind::GuardedStore { access, .. }
                            if access.address_space == AddressSpace::Global =>
                        {
                            result.global_stores += 1
                        }
                        OperationKind::Constant(Constant::U32(value)) => {
                            if !result.literals[..result.literal_count].contains(value) {
                                let slot = result
                                    .literals
                                    .get_mut(result.literal_count)
                                    .ok_or_else(|| resource(ResourceError::Arithmetic))?;
                                *slot = *value;
                                result.literal_count += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    result.literals[..result.literal_count].sort_unstable();
    Ok(result)
}

#[derive(Default)]
struct PendingCallbacks {
    result: Option<Result<PendingObservation, String>>,
}

impl Callbacks for PendingCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut observation = None;
            let result = transaction.observe_pending_scoped_source_v29(|owner, budget| {
                assert!(observation.is_none(), "one complete pending owner");
                observation = Some(inspect(owner, budget)?);
                Ok(())
            });
            match result {
                Err(error)
                    if matches!(*error, PipelineError::PendingScopedObservationIncomplete) =>
                {
                    observation.ok_or_else(|| {
                        "incomplete outcome without constructing and replaying the owner".into()
                    })
                }
                Err(error) => Err(format!(
                    "pending source failed before complete observation: {error:?}"
                )),
                Ok(()) => Err("pending source unexpectedly continued compilation".into()),
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn pending_context_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PendingCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual pending rustc callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("pending observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "pending source observation: {result:?}");
}

fn check_pending_sources(cases: &[(&str, &str)], profiles: &[(u8, u8)]) {
    run_actual_sources::<PendingObservation>(
        cases,
        profiles,
        PENDING_CHILD,
        "PENDING_SOURCE_OBSERVATION",
        source,
        |_, _, label, observation, observations| {
            assert_eq!(observation.kernels, 1);
            assert_eq!(observation.context_issues, 1);
            let derives = usize::from(label != "plain");
            assert_eq!(observation.derives, derives);
            assert_eq!(observation.scope_ends, derives);
            assert_eq!(observation.global_stores, 1);
            assert!(observation.replay_storage_stable);
            assert!(observation.canonical_bytes > 0);
            let expected = match label {
                "constant7" => Some(7),
                "constant11" => Some(11),
                _ => None,
            };
            if let Some(value) = expected {
                assert!(
                    observation.literals[..observation.literal_count].contains(&value),
                    "required value absent from actual pending graph: {observation:?}"
                );
            }
            if let Some(previous) = observations.get(label) {
                assert_eq!(&observation, previous, "fresh-process owner replay");
            } else {
                for previous in observations.values() {
                    assert_ne!(observation.source, previous.source, "changed source digest");
                    assert_ne!(observation.graph, previous.graph, "changed pending graph");
                }
                observations.insert(label.to_owned(), observation);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_plain_source_constructs_and_replays_pending_owner() {
    check_pending_sources(
        &[
            ("plain", "let value = seed;"),
            ("plain", "let value = seed;"),
        ],
        &[(0, 0), (0, 2), (3, 0), (3, 2)],
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_workgroup_sources_construct_and_replay_pending_owner() {
    check_pending_sources(CALLBACK_CASES, &[(0, 0), (0, 2), (3, 0), (3, 2)]);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_scalar_source_constructs_and_replays_pending_owner() {
    run_actual_sources::<PendingObservation>(
        &[("scalar", "let _ = seed;"), ("scalar", "let _ = seed;")],
        &[(0, 0), (0, 2), (3, 0), (3, 2)],
        PENDING_CHILD,
        "PENDING_SCALAR_SOURCE_OBSERVATION",
        |body| {
            format!(
                r#"use fe2o3_device::{{kernel, KernelContext}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn callback_probe(_ctx: KernelContext<'_>, seed: u32) {{
    {body}
}}
"#
            )
        },
        |_, _, label, observation, observations| {
            assert_eq!(observation.kernels, 1);
            assert_eq!(observation.context_issues, 1);
            assert_eq!(observation.derives, 0);
            assert_eq!(observation.scope_ends, 0);
            assert_eq!(observation.global_stores, 0);
            assert!(observation.replay_storage_stable);
            assert!(observation.canonical_bytes > 0);
            if let Some(previous) = observations.get(label) {
                assert_eq!(&observation, previous, "fresh-process scalar owner replay");
            } else {
                observations.insert(label.to_owned(), observation);
            }
        },
    );
}
