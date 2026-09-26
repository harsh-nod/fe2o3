//! Direct V18 execution through the shared storage-aware interpreter.

use super::*;
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrIdentityV18, VerifiedCanonicalKernelIrModuleV18};

/// Completed CPU observation of one exact canonical V18 program.
///
/// This is not a legacy simulator admission, replay transcript, hardware result,
/// or verification authority. The input owner and request are never mutated.
///
/// ```compile_fail
/// use fe2o3_kir_sim::{SimulationExecutionV1, SimulationExecutionV18};
/// fn legacy(result: SimulationExecutionV18) -> SimulationExecutionV1 { result }
/// ```
#[derive(Debug)]
pub struct SimulationExecutionV18 {
    identity: VerifiedCanonicalKernelIrIdentityV18,
    completion: StorageExecutionCompletionV1,
}

impl SimulationExecutionV18 {
    /// Exact identity of the borrowed V18 owner that supplied the executed graph.
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV18 {
        &self.identity
    }

    pub const fn dynamic_workgroup_memory(&self) -> Option<DynamicWorkgroupMemoryRequestV1> {
        self.completion.dynamic_workgroup_memory
    }

    pub fn arguments(&self) -> &[SimulationArgumentV1] {
        &self.completion.arguments
    }

    pub fn buffer(&self, argument: usize) -> Option<&BufferArgumentV1> {
        match self.completion.arguments.get(argument) {
            Some(SimulationArgumentV1::Buffer(buffer)) => Some(buffer),
            _ => None,
        }
    }

    pub fn into_arguments(self) -> Vec<SimulationArgumentV1> {
        self.completion.arguments
    }

    pub fn into_outputs(self) -> (Vec<SimulationArgumentV1>, Vec<SharedBufferV1>) {
        (self.completion.arguments, self.completion.shared_buffers)
    }

    pub fn shared_buffers(&self) -> &[SharedBufferV1] {
        &self.completion.shared_buffers
    }

    pub fn shared_buffer(&self, id: BufferBackingIdV1) -> Option<&BufferArgumentV1> {
        self.completion
            .shared_buffers
            .iter()
            .find(|shared| shared.id == id)
            .map(|shared| &shared.buffer)
    }

    pub const fn invocations_executed(&self) -> u64 {
        self.completion.invocations_executed
    }

    pub const fn workgroups_visited(&self) -> u64 {
        self.completion.workgroups_visited
    }

    pub const fn scheduled_slots_visited(&self) -> u64 {
        self.completion.scheduled_slots_visited
    }

    pub const fn steps_executed(&self) -> u64 {
        self.completion.steps_executed
    }

    pub const fn events_emitted(&self) -> u64 {
        self.completion.events_emitted
    }

    /// Deterministic CPU ordering, not a persisted schedule replay identity.
    pub const fn schedule(&self) -> SimulationScheduleIdentityV1 {
        self.completion.schedule
    }

    pub const fn schedule_coverage(&self) -> SimulationScheduleCoverageV1 {
        self.completion.schedule_coverage
    }

    pub const fn conflict_assessment(&self) -> &SimulationConflictAssessmentV1 {
        &self.completion.conflict_assessment
    }

    /// Bounded race observations from this CPU ordering only.
    pub fn race_assessment(&self) -> &SimulationRaceAssessmentV1 {
        &self
            .completion
            .supplemental
            .first()
            .expect("successful execution retains one race assessment")
            .race_assessment
    }

    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

/// Simulates the actual V18 graph without decoding or converting its identity.
pub fn simulate_canonical_storage_v18(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> Result<SimulationExecutionV18, SimulationErrorV1> {
    simulate_canonical_storage_with_sinks_v18(
        owner,
        request,
        None,
        target,
        limits,
        SimulationDebugCaptureLimitsV1::disabled(),
        &mut NoopSimulationEventSinkV1,
        &mut NoopSimulationDebugSinkV1,
    )
}

/// Simulates V18 with the existing bounded live-state debug callbacks.
///
/// ```no_run
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18;
/// use fe2o3_kir_sim::*;
/// fn observe(owner: &VerifiedCanonicalKernelIrModuleV18, request: &SimulationRequestV1,
///     sink: &mut impl SimulationDebugSinkV1) -> Result<SimulationExecutionV18, SimulationErrorV1> {
///     simulate_canonical_storage_debugged_with_sink_v18(owner, request,
///         SimulationTargetV1::amdgpu_64(), SimulationLimitsV1::default(),
///         SimulationDebugCaptureLimitsV1::new(8, 32, 8, 128).unwrap(), sink)
/// }
/// ```
pub fn simulate_canonical_storage_debugged_with_sink_v18(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    capture: SimulationDebugCaptureLimitsV1,
    debug_sink: &mut impl SimulationDebugSinkV1,
) -> Result<SimulationExecutionV18, SimulationErrorV1> {
    simulate_canonical_storage_with_sinks_v18(
        owner,
        request,
        None,
        target,
        limits,
        capture,
        &mut NoopSimulationEventSinkV1,
        debug_sink,
    )
}

/// Runs V18 with optional explicit dynamic LDS and independent event/debug sinks.
///
/// The same target, ABI, unsupported-operation and resource checks as the shared
/// interpreter run before execution. `request.events` controls event delivery;
/// stopping either sink does not stop execution. Sink-owned retained records
/// remain the caller's responsibility. No legacy schedule replay is accepted.
#[allow(clippy::too_many_arguments)]
pub fn simulate_canonical_storage_with_sinks_v18(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    request: &SimulationRequestV1,
    dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    capture: SimulationDebugCaptureLimitsV1,
    event_sink: &mut impl SimulationEventSinkV1,
    debug_sink: &mut impl SimulationDebugSinkV1,
) -> Result<SimulationExecutionV18, SimulationErrorV1> {
    let limits = limits.validate().map_err(|error| {
        SimulationErrorV1::Preflight(SimulationPreflightErrorV1::InvalidLimits(error))
    })?;
    if owner.canonical_bytes().len() > limits.max_canonical_bytes {
        return Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::ResourceLimit {
                resource: "canonical bytes",
                actual: owner.canonical_bytes().len() as u64,
                limit: limits.max_canonical_bytes as u64,
            },
        ));
    }
    let retained = retained_bytes(owner).ok_or_else(|| {
        SimulationErrorV1::Preflight(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: u64::MAX,
            limit: limits.max_resident_bytes as u64,
        })
    })?;
    let verified = owner.verified_storage_module_ref_v1();
    let plan = crate::preflight::preflight_storage_v1(
        &verified, retained, request, dynamic, target, limits,
    )
    .map_err(SimulationErrorV1::Preflight)?;
    let completion = execute_module_v1(
        owner.module(),
        Some(&verified),
        None,
        request,
        ExecutionConfiguration {
            target,
            limits,
            policy: request.events,
            plan,
            debug_capture: capture,
            schedule: None,
            resident_offset: 0,
        },
        event_sink,
        debug_sink,
    )
    .map_err(SimulationErrorV1::Execution)?;
    Ok(SimulationExecutionV18 {
        identity: *owner.identity(),
        completion,
    })
}

fn retained_bytes(owner: &VerifiedCanonicalKernelIrModuleV18) -> Option<usize> {
    retained_bytes_for_result_v29(owner, size_of::<SimulationExecutionV18>())
}

pub(super) fn retained_bytes_for_result_v29(owner: &VerifiedCanonicalKernelIrModuleV18, result_bytes: usize) -> Option<usize> {
    let view = owner.verified_storage_module_ref_v1();
    let structural = crate::resident::storage_module_retained_bytes_v1(&view)?;
    // Replace the inline Module with its actual owner, retaining its byte buffer.
    // Both V18 admission paths require exact byte capacity. The result wrapper
    // coexists with the shared engine's separately accounted completion slot.
    structural
        .checked_sub(size_of::<Module>())?
        .checked_add(size_of::<VerifiedCanonicalKernelIrModuleV18>())?
        .checked_add(owner.canonical_bytes().len())?
        .checked_add(result_bytes)
}

#[cfg(test)]
#[path = "execute_canonical_storage_v18_tests.rs"]
mod tests;
