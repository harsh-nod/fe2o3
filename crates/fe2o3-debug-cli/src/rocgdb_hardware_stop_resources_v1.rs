//! Sealed producer-owned companion for the existing native V5 register path.
//!
//! Included below `rocgdb_mi_v3::process` so no raw DTO constructor or imported
//! target string can publish a process-owned projection. No GPU is launched here.

use super::*;
use crate::rocgdb_mi_v4::{
    RocgdbCodeObjectBindingV4, RocgdbDirectKfdDeviceBindingV4, RocgdbInferiorBindingV4,
    RocgdbMiNativeCorrelationAdapterV4,
};
use fe2o3_kfd::{CheckedGfx942XnackMinusDevice, KfdTargetDebugTelemetryPayloadV2};

/// In-process inputs retained by the existing launcher, not deserializable input.
pub(crate) struct RocgdbHardwareStopCaptureInputV1<'a> {
    pub(crate) device: &'a CheckedGfx942XnackMinusDevice,
    pub(crate) correlation: &'a RocgdbMiNativeCorrelationAdapterV4,
    pub(crate) declaration: &'a KfdTargetDebugTelemetryPayloadV2,
    pub(crate) publication: &'a KfdTargetDebugTelemetryPayloadV2,
    pub(crate) inferior: RocgdbInferiorBindingV4,
    pub(crate) code: RocgdbCodeObjectBindingV4,
}

/// Move-only and not publicly constructible; the DTO itself is inert.
pub(super) struct RocgdbHardwareStopResourceOwnerV1 {
    projection: RocgdbHardwareStopResourcesV1,
}

impl RocgdbHardwareStopResourceOwnerV1 {
    fn matches(&self, session: OpaqueIdentityV1, stop: RocgdbMiNativeStopPinV4) -> bool {
        self.projection.session_identity == session
            && self.projection.stop_revision == stop.revision
            && self.projection.scope.stop_identity == stop.identity
    }
}

impl RocgdbMiProcessV3 {
    /// Borrowed current-stop data. Copying/serializing it produces historical
    /// evidence, not a device/stop/control capability. No query is issued.
    pub fn native_hardware_stop_resources_v1(&self) -> Option<&RocgdbHardwareStopResourcesV1> {
        let stop = native_stop_pin_v4(&self.adapter, self.native_stop_v4).ok()?;
        let owner = self.hardware_stop_resources_v1.as_ref()?;
        owner
            .matches(self.adapter.session_identity(), stop)
            .then_some(&owner.projection)
    }

    pub(super) fn clear_hardware_stop_resources_v1(&mut self) {
        self.hardware_stop_resources_v1 = None;
    }

    /// Exactly the existing two register MI commands, followed by local bounded
    /// projection. No extra MI command and no change to the returned V5 record.
    pub(crate) fn inspect_native_hardware_registers_v1(
        &mut self,
        input: RocgdbHardwareStopCaptureInputV1<'_>,
        timeout: Duration,
    ) -> Result<(RocgdbMiRegisterSnapshotV3, OpaqueIdentityV1), RocgdbMiAdapterErrorV3> {
        self.clear_hardware_stop_resources_v1();
        let stop = native_stop_pin_v4(&self.adapter, self.native_stop_v4)?;
        if input.correlation.session_identity_v4() != self.adapter.session_identity() {
            return Err(RocgdbMiAdapterErrorV3::StaleRevision);
        }
        // A fresh equality-checked correlation uses the binding derived from
        // this exact checked device, never an MI architecture/browser claim.
        let stopped = input
            .correlation
            .correlate_telemetry(
                input.declaration,
                input.publication,
                RocgdbDirectKfdDeviceBindingV4::from_checked_device(input.device),
                input.inferior,
                input.code,
            )
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        let (raw_thread, scope) = input
            .correlation
            .inspection_scope_v5(&stopped)
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        if scope.stop_identity != stop.identity {
            return Err(RocgdbMiAdapterErrorV3::StaleRevision);
        }
        let captured = self.inspect_native_registers_v5(&raw_thread, scope, timeout)?;
        validate_native_stop_pin_v4(&self.adapter, self.native_stop_v4, stop)?;
        // Failure to retain the additive projection must not alter legacy V5.
        self.hardware_stop_resources_v1 = project_actual_capture(
            checked_target(input.device),
            self.adapter.session_identity(),
            stop,
            &stopped,
            &captured.0,
            captured.1,
        )
        .ok()
        .map(|projection| RocgdbHardwareStopResourceOwnerV1 { projection });
        Ok(captured)
    }

    /// Only the internally generated V5 simple-locals command uses this pure
    /// inspection path. Successful parsing and the final same-stop check are
    /// performed by inspect_native_locals_v5 before data is exposed.
    pub(super) fn send_native_projection_inspection_v1(
        &mut self,
        command: &[u8],
        deadline: Instant,
    ) -> Result<(String, MiResultsV3), RocgdbMiAdapterErrorV3> {
        let before = native_stop_pin_v4(&self.adapter, self.native_stop_v4);
        let result = self.send_command_inner_v1(command, deadline);
        let unchanged = before.ok().is_some_and(|before| {
            validate_native_stop_pin_v4(&self.adapter, self.native_stop_v4, before).is_ok()
        });
        if !unchanged || !matches!(&result, Ok((class, _)) if class == "done") {
            self.clear_hardware_stop_resources_v1();
        }
        result
    }
}

fn checked_target(_device: &CheckedGfx942XnackMinusDevice) -> RocgdbHardwareCheckedTargetV1 {
    // The checked type's admission contract fixes gfx942, xnack-minus and
    // wavefront size 64. The existing launcher has no gfx950 cast or route here.
    RocgdbHardwareCheckedTargetV1::Gfx942XnackMinusWave64
}

fn project_actual_capture(
    target: RocgdbHardwareCheckedTargetV1,
    session_identity: OpaqueIdentityV1,
    stop: RocgdbMiNativeStopPinV4,
    stopped: &RocgdbMiNativeStoppedStateV4,
    snapshot: &RocgdbMiRegisterSnapshotV3,
    evidence: OpaqueIdentityV1,
) -> Result<RocgdbHardwareStopResourcesV1, RocgdbMiAdapterErrorV3> {
    stopped
        .validate()
        .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
    snapshot
        .validate()
        .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
    if stopped.lanes.len() != 64
        || snapshot.scope.lane.is_some()
        || snapshot.scope.stop_identity != stop.identity
        || snapshot.scope.wave.identity != stopped.wave_identity
    {
        return Err(RocgdbMiAdapterErrorV3::ProtocolRecordRejected);
    }
    let registers = project_registers(snapshot, evidence)?;
    let projection = RocgdbHardwareStopResourcesV1 {
        target, session_identity, stop_revision: stop.revision,
        association_identity: stopped.association_identity,
        queue_occurrence_identity: stopped.queue_occurrence_identity,
        process_instance_identity: stopped.process_instance_identity,
        dispatch_identity: stopped.dispatch_identity,
        artifact: stopped.artifact,
        grid: stopped.grid, workgroup: stopped.workgroup,
        workgroup_coordinate: stopped.workgroup_coordinate,
        wave_in_workgroup: stopped.wave_in_workgroup,
        scope: snapshot.scope,
        register_evidence_identity: evidence,
        registers,
        source: RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAuthenticatedSourceMap,
        },
        isa: RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresArtifactRelativeInstructionBinding,
        },
        memory: RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAllocationRelativeAuthority,
        },
    };
    projection
        .validate()
        .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
    Ok(projection)
}

fn project_registers(
    snapshot: &RocgdbMiRegisterSnapshotV3,
    evidence: OpaqueIdentityV1,
) -> Result<RocgdbMiRegisterSnapshotV3, RocgdbMiAdapterErrorV3> {
    if snapshot.registers.len() > MAX_ROCGDB_MI_REGISTERS_V3 {
        return Err(RocgdbMiAdapterErrorV3::ResponseBudgetExhausted);
    }
    let mut registers = Vec::new();
    registers
        .try_reserve_exact(snapshot.registers.len())
        .map_err(|_| RocgdbMiAdapterErrorV3::ResponseBudgetExhausted)?;
    for register in &snapshot.registers {
        if register.lane.is_some() || register.kind != LiveGpuValueKindV3::UnsignedInteger {
            return Err(RocgdbMiAdapterErrorV3::ProtocolRecordRejected);
        }
        let supported = matches!(
            register.class,
            LiveGpuRegisterClassV3::Scalar | LiveGpuRegisterClassV3::Predicate
        );
        let unavailable_value = || unavailable(LiveGpuUnavailableReasonV3::Unsupported);
        let value = match &register.value {
            LiveGpuAvailabilityV3::Available { value, truth } => {
                check_observed(truth, evidence)?;
                match value {
                    LiveGpuValueEncodingV3::Bits { bit_width, bits }
                        if supported && *bit_width > 0 && *bit_width <= 64 =>
                    {
                        LiveGpuAvailabilityV3::Available {
                            value: LiveGpuValueEncodingV3::Bits {
                                bit_width: *bit_width,
                                bits: copy_text(bits, 16)?,
                            },
                            truth: one_observation(evidence)?,
                        }
                    }
                    _ => unavailable_value(),
                }
            }
            LiveGpuAvailabilityV3::Redacted { reason, truth } => {
                check_observed(truth, evidence)?;
                if matches!(register.name.as_str(), "pc" | "pc_all")
                    && *reason == LiveGpuRedactionReasonV3::AbsoluteTargetLocation
                {
                    LiveGpuAvailabilityV3::Redacted {
                        reason: *reason,
                        truth: one_observation(evidence)?,
                    }
                } else {
                    unavailable_value()
                }
            }
            LiveGpuAvailabilityV3::Unavailable { reason, truth } => {
                if truth.origin != LiveGpuTruthOriginV3::Unavailable || !truth.evidence.is_empty() {
                    return Err(RocgdbMiAdapterErrorV3::ProtocolRecordRejected);
                }
                unavailable(if supported {
                    *reason
                } else {
                    LiveGpuUnavailableReasonV3::Unsupported
                })
            }
        };
        registers.push(LiveGpuRegisterValueV3 {
            register_identity: register.register_identity,
            name: copy_text(&register.name, MAX_LIVE_GPU_TEXT_BYTES_V3)?,
            class: register.class,
            kind: register.kind,
            lane: None,
            value,
        });
    }
    Ok(RocgdbMiRegisterSnapshotV3 {
        scope: snapshot.scope,
        registers,
    })
}

fn check_observed(
    truth: &LiveGpuTruthV3,
    evidence: OpaqueIdentityV1,
) -> Result<(), RocgdbMiAdapterErrorV3> {
    if truth.origin != LiveGpuTruthOriginV3::Observed
        || truth.evidence.len() != 1
        || truth.evidence[0].kind != LiveGpuEvidenceKindV3::RuntimeObservation
        || truth.evidence[0].identity != evidence
    {
        return Err(RocgdbMiAdapterErrorV3::ProtocolRecordRejected);
    }
    Ok(())
}

fn one_observation(evidence: OpaqueIdentityV1) -> Result<LiveGpuTruthV3, RocgdbMiAdapterErrorV3> {
    let mut references = Vec::new();
    references
        .try_reserve_exact(1)
        .map_err(|_| RocgdbMiAdapterErrorV3::ResponseBudgetExhausted)?;
    references.push(LiveGpuEvidenceRefV3 {
        kind: LiveGpuEvidenceKindV3::RuntimeObservation,
        identity: evidence,
    });
    Ok(LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Observed,
        evidence: references,
    })
}

fn copy_text(value: &str, limit: usize) -> Result<String, RocgdbMiAdapterErrorV3> {
    if value.len() > limit {
        return Err(RocgdbMiAdapterErrorV3::ResponseBudgetExhausted);
    }
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| RocgdbMiAdapterErrorV3::ResponseBudgetExhausted)?;
    copy.push_str(value);
    Ok(copy)
}

#[cfg(test)]
#[path = "rocgdb_hardware_stop_resources_v1_tests.rs"]
mod tests;
