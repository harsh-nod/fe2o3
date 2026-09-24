//! Bounded public-API acceptance helpers. No private collector or transcript mutation.
use fe2o3_kir_debugger::*;
use fe2o3_kir_sim::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) const BUNDLE_CAP: usize = 256 * 1024;
pub(super) const REPORT_CAP: usize = 1024 * 1024 - 1;
pub(super) const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
pub(super) fn fail(error: impl std::fmt::Display) -> String {
    error.to_string()
}
pub(super) fn debug(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
pub(super) fn demand(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|value| format!("{value:02x}")).collect()
}
pub(super) fn hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub(super) fn limits(workgroup: bool) -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: BUNDLE_CAP,
        max_reachable_functions: 8,
        max_reachable_operations: 512,
        max_invocations: if workgroup { 128 } else { 4 },
        max_workgroups: if workgroup { 2 } else { 1 },
        max_scheduled_slots: 128,
        max_steps: 65_536,
        max_call_depth: 8,
        max_ssa_values: 1024,
        max_allocations: 16,
        max_allocation_bytes: 4096,
        max_total_bytes: 16 * 1024,
        max_resident_bytes: 256 * 1024 * 1024,
        max_events: 262_144,
        max_memory_access_records: 32_768,
    }
}
pub(super) fn options(reuse: bool) -> Result<RuntimeObservationOptionsV1, String> {
    RuntimeObservationOptionsV1::new(
        RuntimeOriginCaptureModeV1::Enabled(
            RuntimeOriginCaptureLimitsV1::new(65_536, 4 * 1024 * 1024).map_err(fail)?,
        ),
        RuntimeFrameCaptureModeV1::Enabled(
            RuntimeFrameCaptureLimitsV1::new(65_536, 131_072, 24 * 1024 * 1024).map_err(fail)?,
        ),
        RuntimeAllocationCaptureModeV1::Enabled(
            RuntimeAllocationCaptureLimitsV1::new_with_validation_work(
                65_536,
                8192,
                8 * 1024 * 1024,
                1_000_000,
            )
            .map_err(fail)?,
        ),
        if reuse {
            Some(SimulationAllocationReuseV1::exact_private_and_workgroup(8192).map_err(fail)?)
        } else {
            None
        },
    )
    .map_err(fail)
}
pub(super) fn schedule(seeded: bool) -> SimulationScheduleRequestV1<'static> {
    if seeded {
        SimulationScheduleRequestV1::RecordSeeded {
            seed: 71,
            max_decisions: 65_536,
        }
    } else {
        SimulationScheduleRequestV1::RecordCanonical {
            max_decisions: 65_536,
        }
    }
}
pub(super) fn capture(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    workgroup: bool,
    seeded: bool,
    reuse: bool,
) -> Result<(SimulationExecutionV1, DebugObservedTranscriptV1), String> {
    let run = capture_debugger_observed_scheduled_run_v1(
        module,
        request,
        TARGET,
        limits(workgroup),
        SimulationDebugCaptureLimitsV1::new(8, 1024, 8, 4096).map_err(fail)?,
        DebuggerLimitsV1::new(65_536, 4_000_000, 256 * 1024 * 1024).map_err(fail)?,
        DebugWaveWidthV1::Wave64,
        options(reuse)?,
        schedule(seeded),
    )
    .map_err(fail)?;
    let (execution, transcript) = run.into_parts();
    let execution = execution.map_err(fail)?;
    demand(
        transcript.legacy().completeness() == DebugTranscriptCompletenessV1::Complete,
        "legacy transcript truncated",
    )?;
    demand(
        transcript.origin_coverage() == RuntimeObservationCoverageV1::Complete,
        "origin metadata incomplete",
    )?;
    demand(
        transcript.frame_coverage() == RuntimeObservationCoverageV1::Complete,
        "frame metadata incomplete",
    )?;
    demand(
        transcript.allocation_coverage() == RuntimeObservationCoverageV1::Complete,
        "lifecycle metadata incomplete",
    )?;
    demand(
        !transcript.legacy().records().is_empty() && transcript.legacy().records().len() <= 65_536,
        "record roster",
    )?;
    Ok((execution, transcript))
}
pub(super) fn work() -> Result<RuntimeReplayWorkV1, String> {
    RuntimeReplayWorkV1::new(1_000_000_000).map_err(debug)
}
pub(super) fn invocation(value: SimulationInvocationV1) -> Value {
    json!({"global":value.global,"workgroup":value.workgroup,"local":value.local,
        "workgroup_size":value.workgroup_size,"workgroup_count":value.workgroup_count,"launch_extent":value.launch_extent})
}
pub(super) fn site(value: SimulationDebugSiteV1) -> Value {
    json!([value.function_ordinal, value.block.0, value.operation])
}
pub(super) fn phase(record: &SimulationDebugRecordV1) -> Option<SimulationDebugCheckpointPhaseV1> {
    match record.kind {
        SimulationDebugRecordKindV1::Checkpoint { phase, .. } => Some(phase),
        _ => None,
    }
}
fn observed_value(value: &SimulationDebugValueV1) -> Value {
    match value {
        SimulationDebugValueV1::PhysicalEntrySymbolicV20(_) => {
            json!({"kind":"unavailable","reason":"not_represented"})
        }
        SimulationDebugValueV1::Scalar(value) => {
            json!({"kind":"scalar","type":format!("{:?}",value.ty()),"bits":format!("{:032x}",value.bits())})
        }
        SimulationDebugValueV1::Pointer {
            allocation,
            byte_offset,
            element,
            address_space,
            access,
            lower_bound,
            upper_bound,
        } => {
            json!({"kind":"pointer","allocation":allocation.to_string(),"offset":byte_offset,"element":format!("{element:?}"),
                "space":format!("{address_space:?}"),"access":format!("{access:?}"),"lower":lower_bound,"upper":upper_bound})
        }
        SimulationDebugValueV1::Slice {
            allocation,
            elements,
            element,
            address_space,
            access,
            byte_offset,
            byte_len,
        } => {
            json!({"kind":"slice","allocation":allocation.to_string(),"elements":elements,"element":format!("{element:?}"),
                "space":format!("{address_space:?}"),"access":format!("{access:?}"),"offset":byte_offset,"bytes":byte_len})
        }
    }
}
pub(super) fn checkpoint(owner: &DebugObservedTranscriptV1, index: usize) -> Result<Value, String> {
    let frames = owner.frames_at(index).map_err(debug)?;
    let origin = owner.origin_at(index).map_err(debug)?;
    render_checkpoint(owner.capture_instance(), frames, origin)
}
pub(super) fn current_checkpoint(session: &DebugObservedSessionV1) -> Result<Value, String> {
    render_checkpoint(
        session.capture_instance(),
        session.current_frames().map_err(debug)?,
        session.current_origin().map_err(debug)?,
    )
}
fn render_checkpoint(
    capture_instance: u64,
    frames: RuntimeFrameObservationV1<'_>,
    origin: RuntimeOriginObservationV1<'_>,
) -> Result<Value, String> {
    let index = usize::try_from(frames.record().ordinal).map_err(fail)?;
    let phase = phase(frames.record()).ok_or("not a checkpoint")?;
    let mut rows = Vec::new();
    for index in 0..frames.len() {
        let frame = frames.get(index).ok_or("missing frame")?;
        let legacy = frame.legacy();
        let SimulationDebugCollectionV1::Captured(values) = &legacy.values else {
            return Err("SSA unavailable".into());
        };
        demand(values.len() <= 256, "selected SSA report cap")?;
        let operation = match frame.operation_state() {
            SimulationDebugFrameOperationV1::Ready => json!({"kind":"ready"}),
            SimulationDebugFrameOperationV1::ActiveOperation {
                attempt,
                site: current,
            } => json!({"kind":"active","attempt":attempt.to_string(),"site":site(current)}),
            SimulationDebugFrameOperationV1::Suspended {
                attempt,
                site: current,
            } => json!({"kind":"suspended","attempt":attempt.to_string(),"site":site(current)}),
        };
        let parent = match frame.parent() {
            SimulationDebugFrameParentV1::Root => json!({"kind":"root"}),
            SimulationDebugFrameParentV1::Caller {
                activation,
                attempt,
                call_site,
            } => {
                json!({"kind":"caller","activation":activation.to_string(),"attempt":attempt.to_string(),"site":site(call_site)})
            }
        };
        rows.push(json!({"depth":legacy.depth,"function":legacy.function_ordinal,"block":legacy.block.0,
            "next_operation":legacy.next_operation,"activation":frame.activation().to_string(),
            "operation":operation,"parent":parent,
            "ssa":values.iter().map(|binding| json!([binding.value.0,observed_value(&binding.observed)])).collect::<Vec<_>>()}));
    }
    Ok(
        json!({"record":index,"ordinal":frames.record().ordinal,"capture_instance":capture_instance.to_string(),
        "invocation":invocation(frames.record().invocation),"site":site(frames.record().site),
        "phase":if phase == SimulationDebugCheckpointPhaseV1::BeforeOperation {"before"}else{"after"},
        "origin":{"activation":origin.activation().to_string(),"attempt":origin.attempt().to_string()},
        "frames":rows}),
    )
}
pub(super) fn identity(value: SimulationAllocationStorageIdentityV1) -> Value {
    json!({"allocation":value.allocation().to_string(),"storage_slot":value.storage_slot().to_string(),
        "generation":value.generation().to_string()})
}
pub(super) fn allocation_scope(value: SimulationAllocationScopeV1) -> Value {
    match value {
        SimulationAllocationScopeV1::Dispatch => json!({"kind":"dispatch"}),
        SimulationAllocationScopeV1::Invocation(value) => {
            json!({"kind":"invocation","invocation":invocation(value)})
        }
        SimulationAllocationScopeV1::Workgroup {
            coordinate,
            size,
            count,
            launch,
        } => {
            json!({"kind":"workgroup","coordinate":coordinate,"size":size,"count":count,"launch":launch})
        }
    }
}
pub(super) fn transition(value: SimulationAllocationTransitionV1) -> Value {
    let descriptor = value.descriptor();
    let kind = match value.kind() {
        SimulationAllocationTransitionKindV1::Preexisting => json!({"kind":"preexisting"}),
        SimulationAllocationTransitionKindV1::Create {
            previous_allocation,
        } => {
            json!({"kind":"create","previous_allocation":previous_allocation.map(|value| value.to_string())})
        }
        SimulationAllocationTransitionKindV1::Release => json!({"kind":"release"}),
    };
    json!({"sequence":value.sequence().to_string(),"identity":identity(descriptor.identity()),
        "kind":kind,"space":format!("{:?}",descriptor.address_space()),"access":format!("{:?}",descriptor.access()),
        "alignment":descriptor.alignment(),"byte_len":descriptor.byte_len(),"scope":allocation_scope(descriptor.scope()),
        "creation_site":descriptor.creation_site().map(|value|json!([value.function_ordinal,value.block.0,value.operation]))})
}
pub(super) fn memory(
    owner: &DebugObservedTranscriptV1,
    index: usize,
    allocation: u64,
) -> Result<&SimulationDebugAllocationV1, String> {
    let record = owner
        .legacy()
        .records()
        .get(index)
        .ok_or("memory record index")?;
    let SimulationDebugRecordKindV1::Checkpoint {
        memory: SimulationDebugCollectionV1::Captured(memory),
        ..
    } = &record.kind
    else {
        return Err("memory snapshot unavailable".into());
    };
    memory
        .iter()
        .find(|value| value.allocation == allocation)
        .ok_or("allocation not in exact snapshot".into())
}
pub(super) fn usage(owner: &DebugObservedTranscriptV1) -> Value {
    let origin = owner.origin_metadata_usage();
    let frames = owner.frame_metadata_usage();
    let allocations = owner.allocation_metadata_usage();
    json!({"origins":{"rows":origin.retained_rows,"capacity":origin.capacity_rows,"bytes":origin.metadata_bytes},
        "frames":{"records":frames.retained_records,"rows":frames.retained_frames,"record_capacity":frames.record_capacity,
            "frame_capacity":frames.frame_capacity,"bytes":frames.metadata_bytes},
        "allocations":{"records":allocations.retained_records,"transitions":allocations.retained_transitions,
            "record_capacity":allocations.record_capacity,"transition_capacity":allocations.transition_capacity,
            "bytes":allocations.metadata_bytes,"validation_work_limit":allocations.validation_work_limit,
            "validation_work_used":allocations.validation_work_used},
        "fixed_owner_bytes":owner.fixed_owner_metadata_bytes()})
}
