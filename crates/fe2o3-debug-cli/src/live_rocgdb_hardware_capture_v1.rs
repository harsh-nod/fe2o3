//! Output adapter after existing one-shot teardown returned (not a reap claim).
//! This module never imports a live owner or issues an MI command.

use super::*;
use fe2o3_debug_protocol::{
    LiveGpuAvailabilityV3, LiveGpuRegisterClassV3, LiveGpuTruthOriginV3,
    LiveGpuUnavailableReasonV3, LiveGpuValueEncodingV3, RocgdbHardwareCaptureErrorV1,
    RocgdbHardwareCaptureLifetimeV1, RocgdbHardwareCaptureResponseV1,
    RocgdbHardwareCaptureResultV1, RocgdbHardwareCaptureSchemaV1,
    RocgdbHardwareCaptureUnavailableV1, RocgdbHardwareLocalsCompletionV1,
    RocgdbHardwareStopResourcesV1, write_rocgdb_hardware_capture_v1,
};

pub(super) fn take_after_final_inspection(
    process: &mut RocgdbMiProcessV3,
    probe: RocgdbMiNativeInspectionProbeV5,
    locals: &RocgdbMiNativeCapturedV5<fe2o3_debug_protocol::RocgdbMiValueSnapshotV3>,
) -> Option<RocgdbHardwareStopResourcesV1> {
    let completed =
        matches!(locals, RocgdbMiNativeCapturedV5::Captured { .. }) && probe.simple_locals;
    let unsupported = matches!(
        locals,
        RocgdbMiNativeCapturedV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::MachineCommandUnavailable,
        }
    ) && !probe.simple_locals;
    if completed || unsupported {
        process.take_historical_hardware_stop_resources_v1()
    } else {
        None
    }
}

pub(super) fn write_historical(
    response: RocgdbMiNativeCliResponseV4,
    inspection_probe: RocgdbMiNativeInspectionProbeV5,
    inspection: Option<RocgdbMiNativeInspectionV5>,
    projection: Option<RocgdbHardwareStopResourcesV1>,
) -> ExitCode {
    // run_inner and the existing Drop path have returned. Drop has no confirmed
    // reap result; historical output must not imply successful process cleanup.
    let Ok(response) = historical_response(
        response_v5(response, inspection_probe, inspection),
        projection,
    ) else {
        return ExitCode::FAILURE;
    };
    if write_rocgdb_hardware_capture_v1(&mut std::io::stdout().lock(), &response).is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn historical_response(
    native: RocgdbMiNativeCliResponseV5,
    projection: Option<RocgdbHardwareStopResourcesV1>,
) -> Result<RocgdbHardwareCaptureResponseV1, RocgdbHardwareCaptureErrorV1> {
    use RocgdbHardwareCaptureErrorV1 as E;
    native.validate().map_err(|_| E::InvalidProjection)?;
    let result = match &native.result {
        RocgdbMiNativeCliResultV5::Unavailable {
            probe,
            inspection_probe,
            reason,
        } => {
            if projection.is_some() {
                return Err(E::InvalidProjection);
            }
            RocgdbHardwareCaptureResultV1::Unavailable {
                probe: *probe,
                inspection_probe: *inspection_probe,
                reason: RocgdbHardwareCaptureUnavailableV1::NativeCapture { reason: *reason },
            }
        }
        RocgdbMiNativeCliResultV5::Available {
            probe,
            inspection_probe,
            stopped_state,
            inspection,
        } => {
            let unavailable = |reason| RocgdbHardwareCaptureResultV1::Unavailable {
                probe: *probe,
                inspection_probe: *inspection_probe,
                reason,
            };
            match &inspection.registers {
                RocgdbMiNativeCapturedV5::Unavailable { reason } => {
                    if projection.is_some() {
                        return Err(E::InvalidProjection);
                    }
                    unavailable(RocgdbHardwareCaptureUnavailableV1::RegisterInspection {
                        reason: *reason,
                    })
                }
                RocgdbMiNativeCapturedV5::Captured {
                    evidence_identity,
                    value: registers,
                } => {
                    let locals_completion = match &inspection.locals {
                        RocgdbMiNativeCapturedV5::Captured { .. } => {
                            Some(RocgdbHardwareLocalsCompletionV1::Captured)
                        }
                        RocgdbMiNativeCapturedV5::Unavailable {
                            reason:
                                RocgdbMiNativeInspectionUnavailableReasonV5::MachineCommandUnavailable,
                        } if !inspection_probe.simple_locals => {
                            Some(RocgdbHardwareLocalsCompletionV1::CommandUnavailable)
                        }
                        RocgdbMiNativeCapturedV5::Unavailable { .. } => None,
                    };
                    match locals_completion {
                        None => {
                            if projection.is_some() {
                                return Err(E::InvalidProjection);
                            }
                            let RocgdbMiNativeCapturedV5::Unavailable { reason } =
                                &inspection.locals
                            else {
                                return Err(E::InvalidProjection);
                            };
                            unavailable(RocgdbHardwareCaptureUnavailableV1::LocalsInspection {
                                reason: *reason,
                            })
                        }
                        Some(locals_completion) => match projection {
                            None => unavailable(
                                RocgdbHardwareCaptureUnavailableV1::ProjectionNotRetained {},
                            ),
                            Some(projection) => {
                                projection.validate().map_err(|_| E::InvalidProjection)?;
                                if !projection_join(
                                    &projection,
                                    stopped_state,
                                    inspection,
                                    registers,
                                    *evidence_identity,
                                ) {
                                    return Err(E::InvalidProjection);
                                }
                                RocgdbHardwareCaptureResultV1::Captured {
                                    probe: *probe,
                                    inspection_probe: *inspection_probe,
                                    locals_completion,
                                    projection,
                                }
                            }
                        },
                    }
                }
            }
        }
    };
    let response = RocgdbHardwareCaptureResponseV1 {
        schema: RocgdbHardwareCaptureSchemaV1::V1,
        observation_lifetime: RocgdbHardwareCaptureLifetimeV1::HistoricalSameStopCapture,
        result,
    };
    response.validate()?;
    Ok(response)
}

fn projection_join(
    projection: &RocgdbHardwareStopResourcesV1,
    stopped: &fe2o3_debug_protocol::RocgdbMiNativeStoppedStateV4,
    inspection: &RocgdbMiNativeInspectionV5,
    registers: &fe2o3_debug_protocol::RocgdbMiRegisterSnapshotV3,
    evidence: OpaqueIdentityV1,
) -> bool {
    if projection.association_identity != stopped.association_identity
        || projection.queue_occurrence_identity != stopped.queue_occurrence_identity
        || projection.process_instance_identity != stopped.process_instance_identity
        || projection.dispatch_identity != stopped.dispatch_identity
        || projection.artifact != stopped.artifact
        || projection.grid != stopped.grid
        || projection.workgroup != stopped.workgroup
        || projection.workgroup_coordinate != stopped.workgroup_coordinate
        || projection.wave_in_workgroup != stopped.wave_in_workgroup
        || projection.scope != inspection.scope
        || projection.scope != registers.scope
        || projection.register_evidence_identity != evidence
        || projection.registers.registers.len() != registers.registers.len()
    {
        return false;
    }
    registers
        .registers
        .iter()
        .zip(&projection.registers.registers)
        .all(|(raw, shown)| {
            if raw.register_identity != shown.register_identity
                || raw.name != shown.name
                || raw.class != shown.class
                || raw.kind != shown.kind
                || raw.lane != shown.lane
            {
                return false;
            }
            let supported = matches!(
                raw.class,
                LiveGpuRegisterClassV3::Scalar | LiveGpuRegisterClassV3::Predicate
            );
            let literal = match &raw.value {
                LiveGpuAvailabilityV3::Available {
                    value: LiveGpuValueEncodingV3::Bits { bit_width, .. },
                    ..
                } => supported && *bit_width > 0 && *bit_width <= 64,
                LiveGpuAvailabilityV3::Unavailable { .. } => supported,
                LiveGpuAvailabilityV3::Redacted { reason, .. } => matches!(
                    raw.name.as_str(),
                    "pc" | "pc_all"
                ) && *reason
                    == fe2o3_debug_protocol::LiveGpuRedactionReasonV3::AbsoluteTargetLocation,
                _ => false,
            };
            if literal {
                shown.value == raw.value
            } else {
                matches!(&shown.value, LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::Unsupported, truth,
            } if truth.origin == LiveGpuTruthOriginV3::Unavailable && truth.evidence.is_empty())
            }
        })
}

#[cfg(test)]
#[path = "live_rocgdb_hardware_capture_v1_tests.rs"]
mod tests;
