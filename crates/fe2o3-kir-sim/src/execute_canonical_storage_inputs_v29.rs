//! Explicit object inputs run through the same actual-owner V18 interpreter.

use super::*;
use crate::storage_inputs_v29::*;
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrIdentityV18, VerifiedCanonicalKernelIrModuleV18};

/// A CPU observation of one exact V18 owner under explicit object inputs.
/// This result is neither a replay request nor a host launch authorization.
///
/// ```compile_fail
/// use fe2o3_kir_sim::{SimulationExecutionV18, SimulationStorageExecutionV29};
/// fn old(result: SimulationStorageExecutionV29) -> SimulationExecutionV18 { result }
/// ```
#[derive(Debug)]
pub struct SimulationStorageExecutionV29 {
    identity: VerifiedCanonicalKernelIrIdentityV18,
    completion: StorageExecutionCompletionV1<
        SimulationStorageArgumentObservationV29,
        SimulationSharedStorageObservationV29,
    >,
}

impl SimulationStorageExecutionV29 {
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV18 {
        &self.identity
    }
    pub const fn dynamic_workgroup_memory(&self) -> Option<DynamicWorkgroupMemoryRequestV1> {
        self.completion.dynamic_workgroup_memory
    }
    pub fn arguments(&self) -> &[SimulationStorageArgumentObservationV29] {
        &self.completion.arguments
    }
    pub fn shared_storage(&self) -> &[SimulationSharedStorageObservationV29] {
        &self.completion.shared_buffers
    }
    pub fn into_outputs(
        self,
    ) -> (
        Vec<SimulationStorageArgumentObservationV29>,
        Vec<SimulationSharedStorageObservationV29>,
    ) {
        (self.completion.arguments, self.completion.shared_buffers)
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
    pub const fn schedule(&self) -> SimulationScheduleIdentityV1 {
        self.completion.schedule
    }
    pub const fn schedule_coverage(&self) -> SimulationScheduleCoverageV1 {
        self.completion.schedule_coverage
    }
    pub const fn conflict_assessment(&self) -> &SimulationConflictAssessmentV1 {
        &self.completion.conflict_assessment
    }
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

pub fn simulate_canonical_storage_inputs_v29(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    request: &SimulationStorageRequestV29,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> Result<SimulationStorageExecutionV29, SimulationErrorV1> {
    simulate_canonical_storage_inputs_with_sinks_v29(
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

pub fn simulate_canonical_storage_inputs_debugged_with_sink_v29(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    request: &SimulationStorageRequestV29,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    capture: SimulationDebugCaptureLimitsV1,
    debug_sink: &mut impl SimulationDebugSinkV1,
) -> Result<SimulationStorageExecutionV29, SimulationErrorV1> {
    simulate_canonical_storage_inputs_with_sinks_v29(
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

#[allow(clippy::too_many_arguments)]
pub fn simulate_canonical_storage_inputs_with_sinks_v29(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    request: &SimulationStorageRequestV29,
    dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    capture: SimulationDebugCaptureLimitsV1,
    event_sink: &mut impl SimulationEventSinkV1,
    debug_sink: &mut impl SimulationDebugSinkV1,
) -> Result<SimulationStorageExecutionV29, SimulationErrorV1> {
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
    let retained = canonical_storage_v18::retained_bytes_for_result_v29(
        owner,
        size_of::<SimulationStorageExecutionV29>(),
    )
    .ok_or_else(|| {
        SimulationErrorV1::Preflight(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: u64::MAX,
            limit: limits.max_resident_bytes as u64,
        })
    })?;
    let verified = owner.verified_storage_module_ref_v1();
    let plan = crate::preflight::preflight_storage_inputs_v29(
        &verified, retained, request, dynamic, target, limits,
    )
    .map_err(SimulationErrorV1::Preflight)?;
    let completion = execute_inputs_v29(
        owner.module(),
        Some(&verified),
        None,
        ExplicitStorageInputsV29(request),
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
    Ok(SimulationStorageExecutionV29 {
        identity: *owner.identity(),
        completion,
    })
}
