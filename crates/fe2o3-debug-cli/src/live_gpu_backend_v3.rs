//! Artifact-bound V3 facade over the production KFD hardware V2 state machine.

use std::fs::File;
use std::io::Read;

use fe2o3_debug_protocol::*;
use fe2o3_kfd::{
    KfdStoppedAvailabilityV1, KfdStoppedSnapshotOwnershipV1, KfdStoppedStateScopeV1,
    KfdStoppedUnavailableReasonV1, KfdTargetDebugArtifactRoleV1, KfdTargetDebugTelemetryPayloadV1,
    KfdTargetDebugTelemetryRecordV1,
};
use sha2::{Digest, Sha256};

use crate::hardware_v2::{
    HardwareBackendV2, HardwareDebugTransportV2, HardwareTransportErrorV2,
    NativeStoppedQueueContextSaveV2, NativeStoppedQueueEnvelopeV2,
    NativeStoppedQueueRelativeRangeV2, StoppedQueueEnvelopeCaptureV2,
};

pub(crate) struct LiveKfdBackendV3<T: HardwareDebugTransportV2> {
    hardware: HardwareBackendV2<T>,
    binding: LiveGpuArtifactBindingV3,
    stopped_scope: LiveKfdStoppedScopeV3,
    limits: LiveGpuProtocolLimitsV3,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LiveKfdStoppedScopeV3(KfdStoppedStateScopeV1);

pub(crate) fn generate_live_kfd_stopped_scope_v3() -> Result<LiveKfdStoppedScopeV3, ()> {
    let mut random = File::open("/dev/urandom").map_err(|_| ())?;
    for _ in 0..2 {
        let mut bytes = [0_u8; 32];
        random.read_exact(&mut bytes).map_err(|_| ())?;
        if let Ok(scope) = KfdStoppedStateScopeV1::new(bytes) {
            return Ok(LiveKfdStoppedScopeV3(scope));
        }
    }
    Err(())
}

impl<T: HardwareDebugTransportV2> LiveKfdBackendV3<T> {
    pub(crate) fn new(
        transport: T,
        binding: LiveGpuArtifactBindingV3,
        stopped_scope: LiveKfdStoppedScopeV3,
    ) -> Self {
        Self {
            hardware: HardwareBackendV2::new(transport),
            binding,
            stopped_scope,
            limits: LiveGpuProtocolLimitsV3::default(),
        }
    }

    pub(crate) const fn limits(&self) -> LiveGpuProtocolLimitsV3 {
        self.limits
    }

    pub(crate) fn apply_target_telemetry(
        &mut self,
        record: &KfdTargetDebugTelemetryRecordV1,
    ) -> Result<(), LiveGpuTelemetryBindingErrorV3> {
        apply_target_telemetry(&mut self.binding, record)
    }

    pub(crate) fn into_transport(self) -> T {
        self.hardware.into_transport()
    }

    pub(crate) fn pump_async_observations(&mut self) -> Result<(), HardwareTransportErrorV2> {
        self.hardware.pump_async_observations()
    }

    pub(crate) fn handle(&mut self, request: LiveGpuDebugRequestV3) -> LiveGpuDebugResponseV3 {
        if let LiveGpuDebugRequestV3::CaptureStoppedQueueEnvelope {
            request_id,
            expected_revision,
            queue,
            ..
        } = &request
        {
            let capture = self.hardware.capture_stopped_queue_envelope(
                *request_id,
                *expected_revision,
                *queue,
                self.stopped_scope.0,
            );
            return convert_stopped_queue_capture(
                capture,
                *request_id,
                &self.binding,
                &mut self.hardware,
            );
        }
        let request_id = request.request_id();
        let operation = request.operation();
        let hardware_request = hardware_request(&request);
        let response = self.hardware.handle(hardware_request);
        convert_response(
            response,
            request_id,
            operation,
            &self.binding,
            &mut self.hardware,
        )
    }
}

fn convert_stopped_queue_capture<T: HardwareDebugTransportV2>(
    capture: StoppedQueueEnvelopeCaptureV2,
    request_id: u64,
    binding: &LiveGpuArtifactBindingV3,
    hardware: &mut HardwareBackendV2<T>,
) -> LiveGpuDebugResponseV3 {
    let operation = LiveGpuOperationV3::CaptureStoppedQueueEnvelope;
    match capture {
        StoppedQueueEnvelopeCaptureV2::Rejected(response) => {
            convert_response(response, request_id, operation, binding, hardware)
        }
        StoppedQueueEnvelopeCaptureV2::Captured {
            session,
            queue,
            device,
            envelope,
        } => {
            let session = live_session(session, binding.binding_identity);
            let Ok(envelope) = project_stopped_queue_envelope(envelope, queue, device) else {
                return backend_protocol_error(
                    request_id,
                    operation,
                    binding.binding_identity,
                    hardware,
                );
            };
            let response = LiveGpuDebugResponseV3::Ok {
                schema: LiveGpuResponseSchemaV3::V3,
                request_id,
                operation,
                session,
                result: Box::new(LiveGpuDebugResultV3::StoppedQueueEnvelope { envelope }),
            };
            if response
                .validate(LiveGpuProtocolLimitsV3::default())
                .is_err()
            {
                return backend_protocol_error(
                    request_id,
                    operation,
                    binding.binding_identity,
                    hardware,
                );
            }
            response
        }
    }
}

fn project_stopped_queue_envelope(
    value: NativeStoppedQueueEnvelopeV2,
    queue: HardwareQueueIdV2,
    device: HardwareDeviceIdV2,
) -> Result<LiveGpuStoppedQueueEnvelopeV3, ()> {
    let envelope_identity = opaque_stopped_identity(value.identity)?;
    let context_save = match value.context_save {
        NativeStoppedQueueContextSaveV2::Available {
            identity,
            context_bytes_per_xcc,
            total_allocation_bytes,
            headers,
        } => LiveGpuStoppedQueueContextSaveV3::Available {
            identity: opaque_stopped_identity(identity)?,
            context_bytes_per_xcc,
            total_allocation_bytes,
            headers: headers
                .into_iter()
                .map(|header| {
                    Ok(LiveGpuStoppedQueueXccHeaderV3 {
                        xcc_ordinal: header.xcc_ordinal,
                        identity: opaque_stopped_identity(header.identity)?,
                        control_stack: stopped_range(header.control_stack),
                        wave_state: stopped_range(header.wave_state),
                        debug: stopped_range(header.debug),
                        error_binding_present: header.error_binding_present,
                    })
                })
                .collect::<Result<_, ()>>()?,
        },
        NativeStoppedQueueContextSaveV2::Unavailable(reason) => {
            LiveGpuStoppedQueueContextSaveV3::Unavailable {
                reason: stopped_reason(reason),
            }
        }
    };
    let ownership = match value.ownership {
        KfdStoppedSnapshotOwnershipV1::SessionRetainedSuspension => {
            LiveGpuStoppedQueueOwnershipV3::SessionRetainedSuspension
        }
    };
    Ok(LiveGpuStoppedQueueEnvelopeV3 {
        envelope_identity,
        queue,
        device,
        queue_observation_identity: opaque_stopped_identity(value.queue_identity)?,
        device_observation_identity: opaque_stopped_identity(value.device_identity)?,
        exception_status_bits: value.exception_status_bits,
        ring_bytes: value.ring_bytes,
        queue_type: value.queue_type,
        gfx_target_version: value.gfx_target_version,
        xcc_count: value.xcc_count,
        ownership,
        resume_required: true,
        context_save,
        hardware_checkpoint_bytes: stopped_unavailable(
            value.hardware_checkpoint_bytes,
            KfdStoppedUnavailableReasonV1::HardwareCheckpointBytesNotCpuVisible,
        )?,
        waves: stopped_unavailable(
            value.waves,
            KfdStoppedUnavailableReasonV1::WaveRecordLayoutNotInKfdUapi,
        )?,
        lanes: stopped_unavailable(
            value.lanes,
            KfdStoppedUnavailableReasonV1::LaneStateRequiresWaveRecords,
        )?,
        registers: stopped_unavailable(
            value.registers,
            KfdStoppedUnavailableReasonV1::RegisterRecordLayoutNotInKfdUapi,
        )?,
        program_counter: stopped_unavailable(
            value.program_counter,
            KfdStoppedUnavailableReasonV1::ProgramCounterRequiresRegisterRecord,
        )?,
        source: stopped_unavailable(
            value.source,
            KfdStoppedUnavailableReasonV1::SourceMapNotBound,
        )?,
        memory: stopped_unavailable(
            value.memory,
            KfdStoppedUnavailableReasonV1::MemoryValuesNotCaptured,
        )?,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Observed,
            evidence: vec![LiveGpuEvidenceRefV3 {
                kind: LiveGpuEvidenceKindV3::RuntimeObservation,
                identity: envelope_identity,
            }],
        },
    })
}

fn opaque_stopped_identity(bytes: [u8; 32]) -> Result<OpaqueIdentityV1, ()> {
    OpaqueIdentityV1::new(bytes).map_err(|_| ())
}

fn stopped_range(value: NativeStoppedQueueRelativeRangeV2) -> LiveGpuStoppedQueueRelativeRangeV3 {
    LiveGpuStoppedQueueRelativeRangeV3 {
        offset: value.offset,
        bytes: value.bytes,
    }
}

fn stopped_unavailable(
    availability: KfdStoppedAvailabilityV1,
    expected: KfdStoppedUnavailableReasonV1,
) -> Result<LiveGpuStoppedQueueUnavailableV3, ()> {
    match availability {
        KfdStoppedAvailabilityV1::Unavailable(reason) if reason == expected => {
            Ok(LiveGpuStoppedQueueUnavailableV3 {
                reason: stopped_reason(reason),
            })
        }
        KfdStoppedAvailabilityV1::Available | KfdStoppedAvailabilityV1::Unavailable(_) => Err(()),
    }
}

fn stopped_reason(reason: KfdStoppedUnavailableReasonV1) -> LiveGpuStoppedQueueUnavailableReasonV3 {
    match reason {
        KfdStoppedUnavailableReasonV1::ContextSaveAreaNotReported => {
            LiveGpuStoppedQueueUnavailableReasonV3::ContextSaveAreaNotReported
        }
        KfdStoppedUnavailableReasonV1::GfxTargetNotGfx942 => {
            LiveGpuStoppedQueueUnavailableReasonV3::GfxTargetNotGfx942
        }
        KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch => {
            LiveGpuStoppedQueueUnavailableReasonV3::Gfx942XccCountMismatch
        }
        KfdStoppedUnavailableReasonV1::Gfx942SaveAreaSizeMismatch => {
            LiveGpuStoppedQueueUnavailableReasonV3::Gfx942SaveAreaSizeMismatch
        }
        KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable => {
            LiveGpuStoppedQueueUnavailableReasonV3::TargetAddressNotRepresentable
        }
        KfdStoppedUnavailableReasonV1::TargetHeaderReadDenied => {
            LiveGpuStoppedQueueUnavailableReasonV3::TargetHeaderReadDenied
        }
        KfdStoppedUnavailableReasonV1::TargetHeaderReadPartial => {
            LiveGpuStoppedQueueUnavailableReasonV3::TargetHeaderReadPartial
        }
        KfdStoppedUnavailableReasonV1::ContextHeaderReservedNonzero => {
            LiveGpuStoppedQueueUnavailableReasonV3::ContextHeaderReservedNonzero
        }
        KfdStoppedUnavailableReasonV1::ContextHeaderRangePairMalformed => {
            LiveGpuStoppedQueueUnavailableReasonV3::ContextHeaderRangePairMalformed
        }
        KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds => {
            LiveGpuStoppedQueueUnavailableReasonV3::ContextHeaderRangeOutOfBounds
        }
        KfdStoppedUnavailableReasonV1::ContextHeaderRangeOverlap => {
            LiveGpuStoppedQueueUnavailableReasonV3::ContextHeaderRangeOverlap
        }
        KfdStoppedUnavailableReasonV1::Gfx942DebugRangeMismatch => {
            LiveGpuStoppedQueueUnavailableReasonV3::Gfx942DebugRangeMismatch
        }
        KfdStoppedUnavailableReasonV1::ContextHeaderBindingSubstituted => {
            LiveGpuStoppedQueueUnavailableReasonV3::ContextHeaderBindingSubstituted
        }
        KfdStoppedUnavailableReasonV1::HardwareCheckpointBytesNotCpuVisible => {
            LiveGpuStoppedQueueUnavailableReasonV3::HardwareCheckpointBytesNotCpuVisible
        }
        KfdStoppedUnavailableReasonV1::WaveRecordLayoutNotInKfdUapi => {
            LiveGpuStoppedQueueUnavailableReasonV3::WaveRecordLayoutNotInKfdUapi
        }
        KfdStoppedUnavailableReasonV1::LaneStateRequiresWaveRecords => {
            LiveGpuStoppedQueueUnavailableReasonV3::LaneStateRequiresWaveRecords
        }
        KfdStoppedUnavailableReasonV1::RegisterRecordLayoutNotInKfdUapi => {
            LiveGpuStoppedQueueUnavailableReasonV3::RegisterRecordLayoutNotInKfdUapi
        }
        KfdStoppedUnavailableReasonV1::ProgramCounterRequiresRegisterRecord => {
            LiveGpuStoppedQueueUnavailableReasonV3::ProgramCounterRequiresRegisterRecord
        }
        KfdStoppedUnavailableReasonV1::SourceMapNotBound => {
            LiveGpuStoppedQueueUnavailableReasonV3::SourceMapNotBound
        }
        KfdStoppedUnavailableReasonV1::MemoryValuesNotCaptured => {
            LiveGpuStoppedQueueUnavailableReasonV3::MemoryValuesNotCaptured
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiveGpuTelemetryBindingErrorV3 {
    MissingSessionStart,
    DuplicateSessionStart,
    DuplicateCodeObject,
    CodeObjectMismatch,
    RecordLimit,
    CountOverflow,
    InvalidEvidenceIdentity,
    UnexpectedPayload,
}

fn apply_target_telemetry(
    binding: &mut LiveGpuArtifactBindingV3,
    record: &KfdTargetDebugTelemetryRecordV1,
) -> Result<(), LiveGpuTelemetryBindingErrorV3> {
    let evidence_identity =
        OpaqueIdentityV1::new(<[u8; 32]>::from(Sha256::digest(record.to_wire_bytes())))
            .map_err(|_| LiveGpuTelemetryBindingErrorV3::InvalidEvidenceIdentity)?;
    let truth = LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Declared,
        evidence: vec![LiveGpuEvidenceRefV3 {
            kind: LiveGpuEvidenceKindV3::Declaration,
            identity: evidence_identity,
        }],
    };
    let first = matches!(
        binding.target_telemetry,
        LiveGpuAvailabilityV3::Unavailable { .. }
    );
    if first
        && !matches!(
            record.payload(),
            KfdTargetDebugTelemetryPayloadV1::SessionStarted { .. }
        )
    {
        return Err(LiveGpuTelemetryBindingErrorV3::MissingSessionStart);
    }
    if !first
        && matches!(
            record.payload(),
            KfdTargetDebugTelemetryPayloadV1::SessionStarted { .. }
        )
    {
        return Err(LiveGpuTelemetryBindingErrorV3::DuplicateSessionStart);
    }
    let mut summary = match &binding.target_telemetry {
        LiveGpuAvailabilityV3::Available { value, .. } => value.clone(),
        LiveGpuAvailabilityV3::Unavailable { .. } | LiveGpuAvailabilityV3::Redacted { .. } => {
            LiveGpuTargetTelemetrySummaryV3 {
                records: 0,
                artifact_records: 0,
                dispatch_records: 0,
                allocation_records: 0,
                diagnostic_records: 0,
                session_ended: false,
            }
        }
    };
    summary.records = summary
        .records
        .checked_add(1)
        .ok_or(LiveGpuTelemetryBindingErrorV3::CountOverflow)?;
    if summary.records > MAX_LIVE_GPU_TARGET_TELEMETRY_RECORDS_V3 {
        return Err(LiveGpuTelemetryBindingErrorV3::RecordLimit);
    }
    match record.payload() {
        KfdTargetDebugTelemetryPayloadV1::SessionStarted { .. } => {}
        KfdTargetDebugTelemetryPayloadV1::Artifact { role, artifact, .. } => {
            summary.artifact_records = summary
                .artifact_records
                .checked_add(1)
                .ok_or(LiveGpuTelemetryBindingErrorV3::CountOverflow)?;
            if *role == KfdTargetDebugArtifactRoleV1::CodeObject {
                if matches!(
                    binding.target_declared_code_object,
                    LiveGpuAvailabilityV3::Available { .. }
                ) {
                    return Err(LiveGpuTelemetryBindingErrorV3::DuplicateCodeObject);
                }
                let content = LiveGpuContentIdentityV3 {
                    digest: OpaqueIdentityV1::new(*artifact.digest().as_bytes())
                        .map_err(|_| LiveGpuTelemetryBindingErrorV3::InvalidEvidenceIdentity)?,
                    canonical_bytes: artifact.byte_length(),
                };
                if content != binding.declared_code_object {
                    return Err(LiveGpuTelemetryBindingErrorV3::CodeObjectMismatch);
                }
                binding.target_declared_code_object = LiveGpuAvailabilityV3::Available {
                    value: content,
                    truth: truth.clone(),
                };
            }
        }
        KfdTargetDebugTelemetryPayloadV1::Dispatch { .. } => {
            summary.dispatch_records = summary
                .dispatch_records
                .checked_add(1)
                .ok_or(LiveGpuTelemetryBindingErrorV3::CountOverflow)?;
        }
        KfdTargetDebugTelemetryPayloadV1::Allocation { .. } => {
            summary.allocation_records = summary
                .allocation_records
                .checked_add(1)
                .ok_or(LiveGpuTelemetryBindingErrorV3::CountOverflow)?;
        }
        KfdTargetDebugTelemetryPayloadV1::Diagnostic { .. } => {
            summary.diagnostic_records = summary
                .diagnostic_records
                .checked_add(1)
                .ok_or(LiveGpuTelemetryBindingErrorV3::CountOverflow)?;
        }
        KfdTargetDebugTelemetryPayloadV1::SessionEnded { .. } => {
            summary.session_ended = true;
        }
        _ => return Err(LiveGpuTelemetryBindingErrorV3::UnexpectedPayload),
    }
    binding.target_telemetry = LiveGpuAvailabilityV3::Available {
        value: summary,
        truth,
    };
    Ok(())
}

fn hardware_request(request: &LiveGpuDebugRequestV3) -> HardwareDebugRequestV2 {
    let request_id = request.request_id();
    let expected_control_revision = request.expected_revision();
    match request {
        LiveGpuDebugRequestV3::DiscoverCapabilities { .. } => {
            HardwareDebugRequestV2::DiscoverCapabilities {
                schema: HardwareRequestSchemaV2::V2,
                request_id,
                expected_control_revision,
            }
        }
        LiveGpuDebugRequestV3::InspectHardwareDevices { page, .. } => {
            HardwareDebugRequestV2::InspectHardwareDevices {
                schema: HardwareRequestSchemaV2::V2,
                request_id,
                expected_control_revision,
                page: *page,
            }
        }
        LiveGpuDebugRequestV3::InspectHardwareQueues { page, .. } => {
            HardwareDebugRequestV2::InspectHardwareQueues {
                schema: HardwareRequestSchemaV2::V2,
                request_id,
                expected_control_revision,
                page: *page,
            }
        }
        LiveGpuDebugRequestV3::QueryHardwareExceptionEvents { page, .. } => {
            HardwareDebugRequestV2::QueryHardwareExceptionEvents {
                schema: HardwareRequestSchemaV2::V2,
                request_id,
                expected_control_revision,
                page: *page,
            }
        }
        LiveGpuDebugRequestV3::SuspendQueues {
            queues,
            grace_period,
            ..
        } => HardwareDebugRequestV2::SuspendQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id,
            expected_control_revision,
            queues: queues.clone(),
            grace_period: *grace_period,
        },
        LiveGpuDebugRequestV3::ResumeQueues { queues, .. } => {
            HardwareDebugRequestV2::ResumeQueues {
                schema: HardwareRequestSchemaV2::V2,
                request_id,
                expected_control_revision,
                queues: queues.clone(),
            }
        }
        LiveGpuDebugRequestV3::Terminate { .. } => HardwareDebugRequestV2::Terminate {
            schema: HardwareRequestSchemaV2::V2,
            request_id,
            expected_control_revision,
        },
        LiveGpuDebugRequestV3::GetSessionBinding { .. }
        | LiveGpuDebugRequestV3::GetState { .. }
        | LiveGpuDebugRequestV3::CaptureStoppedQueueEnvelope { .. }
        | LiveGpuDebugRequestV3::InspectStoppedScopes { .. }
        | LiveGpuDebugRequestV3::InspectRegisters { .. }
        | LiveGpuDebugRequestV3::InspectValues { .. }
        | LiveGpuDebugRequestV3::ReadMemory { .. }
        | LiveGpuDebugRequestV3::ResolveProgramSite { .. }
        | LiveGpuDebugRequestV3::InsertBreakpoint { .. }
        | LiveGpuDebugRequestV3::RemoveBreakpoint { .. }
        | LiveGpuDebugRequestV3::Continue { .. }
        | LiveGpuDebugRequestV3::Pause { .. }
        | LiveGpuDebugRequestV3::Step { .. } => HardwareDebugRequestV2::GetState {
            schema: HardwareRequestSchemaV2::V2,
            request_id,
            expected_control_revision,
        },
    }
}

fn convert_response<T: HardwareDebugTransportV2>(
    response: HardwareDebugResponseV2,
    request_id: u64,
    operation: LiveGpuOperationV3,
    binding: &LiveGpuArtifactBindingV3,
    hardware: &mut HardwareBackendV2<T>,
) -> LiveGpuDebugResponseV3 {
    match response {
        HardwareDebugResponseV2::Ok {
            session, result, ..
        } => {
            let session = live_session(session, binding.binding_identity);
            let result = match operation {
                LiveGpuOperationV3::DiscoverCapabilities => {
                    let HardwareDebugResultV2::Capabilities { capabilities } = result else {
                        return backend_protocol_error(
                            request_id,
                            operation,
                            binding.binding_identity,
                            hardware,
                        );
                    };
                    LiveGpuDebugResultV3::Capabilities {
                        capabilities: live_capabilities(&capabilities, binding),
                    }
                }
                LiveGpuOperationV3::GetSessionBinding => {
                    if !matches!(result, HardwareDebugResultV2::State) {
                        return backend_protocol_error(
                            request_id,
                            operation,
                            binding.binding_identity,
                            hardware,
                        );
                    }
                    LiveGpuDebugResultV3::SessionBinding {
                        binding: binding.clone(),
                    }
                }
                LiveGpuOperationV3::GetState => {
                    if !matches!(result, HardwareDebugResultV2::State) {
                        return backend_protocol_error(
                            request_id,
                            operation,
                            binding.binding_identity,
                            hardware,
                        );
                    }
                    LiveGpuDebugResultV3::State {
                        stopped: LiveGpuAvailabilityV3::Unavailable {
                            reason: LiveGpuUnavailableReasonV3::SessionNotStopped,
                            truth: unavailable_truth(),
                        },
                    }
                }
                LiveGpuOperationV3::InspectHardwareDevices
                | LiveGpuOperationV3::InspectHardwareQueues
                | LiveGpuOperationV3::QueryHardwareExceptionEvents
                | LiveGpuOperationV3::SuspendQueues
                | LiveGpuOperationV3::ResumeQueues => {
                    LiveGpuDebugResultV3::Hardware { hardware: result }
                }
                LiveGpuOperationV3::Terminate => {
                    if !matches!(result, HardwareDebugResultV2::Terminated) {
                        return backend_protocol_error(
                            request_id,
                            operation,
                            binding.binding_identity,
                            hardware,
                        );
                    }
                    LiveGpuDebugResultV3::Terminated
                }
                LiveGpuOperationV3::InspectStoppedScopes
                | LiveGpuOperationV3::InspectRegisters
                | LiveGpuOperationV3::InspectValues
                | LiveGpuOperationV3::ReadMemory
                | LiveGpuOperationV3::ResolveProgramSite
                | LiveGpuOperationV3::InsertBreakpoint
                | LiveGpuOperationV3::RemoveBreakpoint
                | LiveGpuOperationV3::Continue
                | LiveGpuOperationV3::Pause
                | LiveGpuOperationV3::Step => {
                    return LiveGpuDebugResponseV3::Unavailable {
                        schema: LiveGpuResponseSchemaV3::V3,
                        request_id,
                        operation,
                        session,
                        reason: LiveGpuUnavailableReasonV3::Unsupported,
                    };
                }
                LiveGpuOperationV3::CaptureStoppedQueueEnvelope => {
                    return backend_protocol_error(
                        request_id,
                        operation,
                        binding.binding_identity,
                        hardware,
                    );
                }
            };
            LiveGpuDebugResponseV3::Ok {
                schema: LiveGpuResponseSchemaV3::V3,
                request_id,
                operation,
                session,
                result: Box::new(result),
            }
        }
        HardwareDebugResponseV2::Unavailable {
            session, reason, ..
        } => LiveGpuDebugResponseV3::Unavailable {
            schema: LiveGpuResponseSchemaV3::V3,
            request_id,
            operation,
            session: live_session(session, binding.binding_identity),
            reason: match reason {
                HardwareUnavailableReasonV2::NotProvidedByKfd
                | HardwareUnavailableReasonV2::DeviceCapabilityAbsent
                | HardwareUnavailableReasonV2::GeneralLaunchUnavailable => {
                    LiveGpuUnavailableReasonV3::Unsupported
                }
                HardwareUnavailableReasonV2::RuntimeNotEnabled => {
                    LiveGpuUnavailableReasonV3::BackendNotConnected
                }
            },
        },
        HardwareDebugResponseV2::Error { session, error, .. } => LiveGpuDebugResponseV3::Error {
            schema: LiveGpuResponseSchemaV3::V3,
            request_id: Some(request_id),
            operation: Some(operation),
            session: live_session(session, binding.binding_identity),
            error: LiveGpuErrorV3 {
                stage: match error.stage {
                    HardwareErrorStageV2::Framing => LiveGpuErrorStageV3::Framing,
                    HardwareErrorStageV2::Validation => LiveGpuErrorStageV3::Validation,
                    HardwareErrorStageV2::Snapshot | HardwareErrorStageV2::Event => {
                        LiveGpuErrorStageV3::Observation
                    }
                    HardwareErrorStageV2::Session | HardwareErrorStageV2::Control => {
                        LiveGpuErrorStageV3::Query
                    }
                    HardwareErrorStageV2::Cleanup | HardwareErrorStageV2::Output => {
                        LiveGpuErrorStageV3::Output
                    }
                },
                code: match error.code {
                    HardwareErrorCodeV2::InvalidJson | HardwareErrorCodeV2::InvalidRequest => {
                        LiveGpuErrorCodeV3::InvalidRequest
                    }
                    HardwareErrorCodeV2::StaleControlRevision => LiveGpuErrorCodeV3::StaleRevision,
                    HardwareErrorCodeV2::StaleIdentityGeneration
                    | HardwareErrorCodeV2::UnknownLogicalId => {
                        LiveGpuErrorCodeV3::UnknownLogicalIdentity
                    }
                    HardwareErrorCodeV2::StaleEventCursor => LiveGpuErrorCodeV3::StaleSnapshot,
                    HardwareErrorCodeV2::RuntimeNotEnabled
                    | HardwareErrorCodeV2::BackendFailure => LiveGpuErrorCodeV3::BackendFailure,
                    HardwareErrorCodeV2::ResourceLimit => LiveGpuErrorCodeV3::ResourceLimit,
                    HardwareErrorCodeV2::SessionPoisoned => LiveGpuErrorCodeV3::SessionPoisoned,
                    HardwareErrorCodeV2::SessionTerminated => LiveGpuErrorCodeV3::SessionTerminated,
                    HardwareErrorCodeV2::ResponseTooLarge => LiveGpuErrorCodeV3::ResponseTooLarge,
                },
                effect: error.effect,
                terminal: error.terminal,
            },
        },
    }
}

fn live_session(
    session: HardwareSessionViewV2,
    binding_identity: OpaqueIdentityV1,
) -> LiveGpuSessionViewV3 {
    LiveGpuSessionViewV3 {
        backend: LiveGpuBackendV3::DirectKfd,
        state: match session.state {
            HardwareSessionStateV2::Running => LiveGpuSessionStateV3::Running,
            HardwareSessionStateV2::Poisoned => LiveGpuSessionStateV3::Poisoned,
            HardwareSessionStateV2::Terminated => LiveGpuSessionStateV3::Terminated,
        },
        revision: session.control_revision,
        commands_processed: session.commands_processed,
        observation_sequence: session.observation_sequence,
        identity_generation: session.identity_generation,
        runtime_enabled: session.runtime_enabled,
        binding_identity,
    }
}

fn live_capabilities(
    hardware: &[HardwareCapabilityV2],
    binding: &LiveGpuArtifactBindingV3,
) -> Vec<LiveGpuCapabilityV3> {
    use HardwareCapabilityNameV2 as Hardware;
    use LiveGpuCapabilityNameV3 as Live;

    let mut capabilities = vec![
        live_capability(Live::ExactArtifactBinding, true, None),
        live_capability(Live::CooperativeTargetTelemetry, true, None),
        live_capability(
            Live::CpuReferenceEvidence,
            matches!(
                binding.cpu_reference.deterministic_evidence,
                LiveGpuCpuReferenceEvidenceV3::Available { .. }
            ),
            Some(LiveGpuUnavailableReasonV3::NotCaptured),
        ),
    ];
    for (live, name) in [
        (
            Live::HardwareDeviceSnapshot,
            Hardware::HardwareDeviceSnapshot,
        ),
        (Live::HardwareQueueSnapshot, Hardware::HardwareQueueSnapshot),
        (
            Live::HardwareExceptionEvents,
            Hardware::HardwareExceptionEvents,
        ),
        (Live::QueueSuspend, Hardware::QueueSuspend),
        (Live::QueueResume, Hardware::QueueResume),
        (Live::Terminate, Hardware::Terminate),
    ] {
        let available = hardware.iter().any(|capability| {
            capability.name == name
                && capability.availability == HardwareCapabilityAvailabilityV2::Available
        });
        capabilities.push(live_capability(
            live,
            available,
            Some(LiveGpuUnavailableReasonV3::Unsupported),
        ));
    }
    let stopped_queue_envelope_available = [
        Hardware::HardwareDeviceSnapshot,
        Hardware::HardwareQueueSnapshot,
        Hardware::QueueSuspend,
        Hardware::QueueResume,
    ]
    .into_iter()
    .all(|name| {
        hardware.iter().any(|capability| {
            capability.name == name
                && capability.availability == HardwareCapabilityAvailabilityV2::Available
        })
    });
    capabilities.push(live_capability(
        Live::StoppedQueueEnvelope,
        stopped_queue_envelope_available,
        Some(LiveGpuUnavailableReasonV3::Unsupported),
    ));
    for name in [
        Live::StoppedDispatch,
        Live::StoppedWorkgroups,
        Live::StoppedWaves,
        Live::StoppedLanes,
        Live::RelativeProgramCounter,
        Live::IsaSite,
        Live::KirSite,
        Live::SourceSite,
        Live::RegisterValues,
        Live::SemanticValues,
        Live::AllocationRelativeMemory,
        Live::Breakpoints,
        Live::Continue,
        Live::Pause,
        Live::Step,
    ] {
        capabilities.push(live_capability(
            name,
            false,
            Some(LiveGpuUnavailableReasonV3::Unsupported),
        ));
    }
    capabilities
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    #[test]
    fn direct_kfd_capability_classification_is_exhaustive() {
        fn is_classified(name: LiveGpuCapabilityNameV3) -> bool {
            match name {
                LiveGpuCapabilityNameV3::ExactArtifactBinding
                | LiveGpuCapabilityNameV3::CooperativeTargetTelemetry
                | LiveGpuCapabilityNameV3::CpuReferenceEvidence
                | LiveGpuCapabilityNameV3::HardwareDeviceSnapshot
                | LiveGpuCapabilityNameV3::HardwareQueueSnapshot
                | LiveGpuCapabilityNameV3::HardwareExceptionEvents
                | LiveGpuCapabilityNameV3::QueueSuspend
                | LiveGpuCapabilityNameV3::QueueResume
                | LiveGpuCapabilityNameV3::StoppedQueueEnvelope
                | LiveGpuCapabilityNameV3::Terminate
                | LiveGpuCapabilityNameV3::StoppedDispatch
                | LiveGpuCapabilityNameV3::StoppedWorkgroups
                | LiveGpuCapabilityNameV3::StoppedWaves
                | LiveGpuCapabilityNameV3::StoppedLanes
                | LiveGpuCapabilityNameV3::RelativeProgramCounter
                | LiveGpuCapabilityNameV3::IsaSite
                | LiveGpuCapabilityNameV3::KirSite
                | LiveGpuCapabilityNameV3::SourceSite
                | LiveGpuCapabilityNameV3::RegisterValues
                | LiveGpuCapabilityNameV3::SemanticValues
                | LiveGpuCapabilityNameV3::AllocationRelativeMemory
                | LiveGpuCapabilityNameV3::Breakpoints
                | LiveGpuCapabilityNameV3::Continue
                | LiveGpuCapabilityNameV3::Pause
                | LiveGpuCapabilityNameV3::Step => true,
            }
        }

        for capability in [
            LiveGpuCapabilityNameV3::ExactArtifactBinding,
            LiveGpuCapabilityNameV3::CooperativeTargetTelemetry,
            LiveGpuCapabilityNameV3::CpuReferenceEvidence,
            LiveGpuCapabilityNameV3::HardwareDeviceSnapshot,
            LiveGpuCapabilityNameV3::HardwareQueueSnapshot,
            LiveGpuCapabilityNameV3::HardwareExceptionEvents,
            LiveGpuCapabilityNameV3::QueueSuspend,
            LiveGpuCapabilityNameV3::QueueResume,
            LiveGpuCapabilityNameV3::StoppedQueueEnvelope,
            LiveGpuCapabilityNameV3::Terminate,
            LiveGpuCapabilityNameV3::StoppedDispatch,
            LiveGpuCapabilityNameV3::StoppedWorkgroups,
            LiveGpuCapabilityNameV3::StoppedWaves,
            LiveGpuCapabilityNameV3::StoppedLanes,
            LiveGpuCapabilityNameV3::RelativeProgramCounter,
            LiveGpuCapabilityNameV3::IsaSite,
            LiveGpuCapabilityNameV3::KirSite,
            LiveGpuCapabilityNameV3::SourceSite,
            LiveGpuCapabilityNameV3::RegisterValues,
            LiveGpuCapabilityNameV3::SemanticValues,
            LiveGpuCapabilityNameV3::AllocationRelativeMemory,
            LiveGpuCapabilityNameV3::Breakpoints,
            LiveGpuCapabilityNameV3::Continue,
            LiveGpuCapabilityNameV3::Pause,
            LiveGpuCapabilityNameV3::Step,
        ] {
            assert!(is_classified(capability));
        }
    }
}

fn live_capability(
    name: LiveGpuCapabilityNameV3,
    available: bool,
    unavailable_reason: Option<LiveGpuUnavailableReasonV3>,
) -> LiveGpuCapabilityV3 {
    LiveGpuCapabilityV3 {
        backend: LiveGpuBackendV3::DirectKfd,
        name,
        availability: if available {
            LiveGpuCapabilityAvailabilityV3::Available
        } else {
            LiveGpuCapabilityAvailabilityV3::Unavailable
        },
        unavailable_reason: if available { None } else { unavailable_reason },
    }
}

fn unavailable_truth() -> LiveGpuTruthV3 {
    LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Unavailable,
        evidence: Vec::new(),
    }
}

fn backend_protocol_error<T: HardwareDebugTransportV2>(
    request_id: u64,
    operation: LiveGpuOperationV3,
    binding_identity: OpaqueIdentityV1,
    hardware: &mut HardwareBackendV2<T>,
) -> LiveGpuDebugResponseV3 {
    let session = live_session(hardware.poison_protocol_error(), binding_identity);
    LiveGpuDebugResponseV3::Error {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: Some(request_id),
        operation: Some(operation),
        session,
        error: LiveGpuErrorV3 {
            stage: LiveGpuErrorStageV3::Observation,
            code: LiveGpuErrorCodeV3::BackendFailure,
            effect: HardwareEffectV2::Indeterminate,
            terminal: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware_v2::{
        NativeDeviceV2, NativeEventV2, NativeExceptionInfoV2, NativeQueueOutcomeV2, NativeQueueV2,
        NativeStoppedQueueCaptureErrorV2,
    };
    use fe2o3_kfd::{
        KfdTargetDebugArtifactIdentityV1, KfdTargetDebugSessionNonceV1,
        KfdTargetDebugTelemetryDigestV1,
    };
    use fe2o3_kfd_uapi::KfdDebugTrapExceptionCodeV1;

    #[derive(Default)]
    struct EmptyTransportV3;

    impl HardwareDebugTransportV2 for EmptyTransportV3 {
        fn device_snapshot(&mut self) -> Result<Vec<NativeDeviceV2>, HardwareTransportErrorV2> {
            Ok(Vec::new())
        }

        fn queue_snapshot(&mut self) -> Result<Vec<NativeQueueV2>, HardwareTransportErrorV2> {
            Ok(Vec::new())
        }

        fn drain_notifications(&mut self) -> Result<(), HardwareTransportErrorV2> {
            Ok(())
        }

        fn query_event(&mut self) -> Result<Option<NativeEventV2>, HardwareTransportErrorV2> {
            Ok(None)
        }

        fn query_exception_info(
            &mut self,
            _source_id: u32,
            _code: KfdDebugTrapExceptionCodeV1,
            _clear: bool,
        ) -> Result<NativeExceptionInfoV2, HardwareTransportErrorV2> {
            unreachable!("empty transport has no events")
        }

        fn acknowledge_runtime_transition(
            &mut self,
            _event: NativeEventV2,
        ) -> Result<(), HardwareTransportErrorV2> {
            unreachable!("empty transport has no runtime transition")
        }

        fn suspend_queues(
            &mut self,
            _queues: &[u32],
            _grace_period: u32,
        ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2> {
            unreachable!("empty transport has no queues")
        }

        fn resume_queues(
            &mut self,
            _queues: &[u32],
        ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2> {
            unreachable!("empty transport has no queues")
        }

        fn capture_stopped_queue(
            &mut self,
            _queue: u32,
            _scope: KfdStoppedStateScopeV1,
        ) -> Result<NativeStoppedQueueEnvelopeV2, NativeStoppedQueueCaptureErrorV2> {
            unreachable!("empty transport has no suspended queues")
        }
    }

    fn identity(seed: u8) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new([seed; 32]).unwrap()
    }

    fn test_scope(seed: u8) -> LiveKfdStoppedScopeV3 {
        LiveKfdStoppedScopeV3(KfdStoppedStateScopeV1::new([seed; 32]).unwrap())
    }

    fn content(seed: u8) -> LiveGpuContentIdentityV3 {
        LiveGpuContentIdentityV3 {
            digest: identity(seed),
            canonical_bytes: 64,
        }
    }

    fn binding() -> LiveGpuArtifactBindingV3 {
        LiveGpuArtifactBindingV3 {
            binding_identity: identity(1),
            code_object_version: 6,
            declared_code_object: content(2),
            declaration: LiveGpuTruthV3 {
                origin: LiveGpuTruthOriginV3::Declared,
                evidence: vec![LiveGpuEvidenceRefV3 {
                    kind: LiveGpuEvidenceKindV3::Declaration,
                    identity: identity(3),
                }],
            },
            target_declared_code_object: LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::NotObserved,
                truth: unavailable_truth(),
            },
            target_telemetry: LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::NotObserved,
                truth: unavailable_truth(),
            },
            execution_code_object: LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::NotObserved,
                truth: unavailable_truth(),
            },
            kernel_ir_v7: content(4),
            source_map_v2: content(5),
            isa_map_v1: None,
            cpu_reference: LiveGpuCpuReferenceBindingV3 {
                bundle_identity: identity(6),
                request_identity: identity(7),
                configuration_identity: identity(8),
                deterministic_evidence: LiveGpuCpuReferenceEvidenceV3::Unavailable {
                    reason: LiveGpuUnavailableReasonV3::NotCaptured,
                },
            },
        }
    }

    fn native_unavailable(reason: KfdStoppedUnavailableReasonV1) -> KfdStoppedAvailabilityV1 {
        KfdStoppedAvailabilityV1::Unavailable(reason)
    }

    fn native_stopped_envelope() -> NativeStoppedQueueEnvelopeV2 {
        NativeStoppedQueueEnvelopeV2 {
            identity: [61; 32],
            queue_identity: [62; 32],
            device_identity: [63; 32],
            exception_status_bits: 0x20,
            ring_bytes: 4096,
            queue_type: 0,
            gfx_target_version: 90_402,
            xcc_count: 8,
            ownership: KfdStoppedSnapshotOwnershipV1::SessionRetainedSuspension,
            context_save: NativeStoppedQueueContextSaveV2::Unavailable(
                KfdStoppedUnavailableReasonV1::TargetHeaderReadDenied,
            ),
            hardware_checkpoint_bytes: native_unavailable(
                KfdStoppedUnavailableReasonV1::HardwareCheckpointBytesNotCpuVisible,
            ),
            waves: native_unavailable(KfdStoppedUnavailableReasonV1::WaveRecordLayoutNotInKfdUapi),
            lanes: native_unavailable(KfdStoppedUnavailableReasonV1::LaneStateRequiresWaveRecords),
            registers: native_unavailable(
                KfdStoppedUnavailableReasonV1::RegisterRecordLayoutNotInKfdUapi,
            ),
            program_counter: native_unavailable(
                KfdStoppedUnavailableReasonV1::ProgramCounterRequiresRegisterRecord,
            ),
            source: native_unavailable(KfdStoppedUnavailableReasonV1::SourceMapNotBound),
            memory: native_unavailable(KfdStoppedUnavailableReasonV1::MemoryValuesNotCaptured),
        }
    }

    #[test]
    fn stopped_queue_projection_preserves_redaction_and_rejects_claim_upgrades() {
        let queue = HardwareQueueIdV2 {
            generation: 1,
            ordinal: 2,
        };
        let device = HardwareDeviceIdV2 {
            generation: 1,
            ordinal: 3,
        };
        let projected =
            project_stopped_queue_envelope(native_stopped_envelope(), queue, device).unwrap();
        assert_eq!(projected.queue, queue);
        assert_eq!(projected.device, device);
        assert_eq!(projected.exception_status_bits, 0x20);
        assert!(projected.resume_required);
        assert!(matches!(
            projected.context_save,
            LiveGpuStoppedQueueContextSaveV3::Unavailable {
                reason: LiveGpuStoppedQueueUnavailableReasonV3::TargetHeaderReadDenied,
            }
        ));
        assert_eq!(
            projected.truth,
            LiveGpuTruthV3 {
                origin: LiveGpuTruthOriginV3::Observed,
                evidence: vec![LiveGpuEvidenceRefV3 {
                    kind: LiveGpuEvidenceKindV3::RuntimeObservation,
                    identity: projected.envelope_identity,
                }],
            }
        );

        let mut hostile = native_stopped_envelope();
        hostile.waves = KfdStoppedAvailabilityV1::Available;
        assert!(project_stopped_queue_envelope(hostile, queue, device).is_err());

        let mut zero_identity = native_stopped_envelope();
        zero_identity.queue_identity = [0; 32];
        assert!(project_stopped_queue_envelope(zero_identity, queue, device).is_err());
    }

    #[test]
    fn stopped_scopes_are_fresh_private_and_not_binding_derived() {
        let first = generate_live_kfd_stopped_scope_v3().unwrap();
        let second = generate_live_kfd_stopped_scope_v3().unwrap();
        assert_ne!(first, second);
        assert_eq!(
            format!("{:?}", first.0),
            "KfdStoppedStateScopeV1(<redacted>)"
        );
        let source = include_str!("live_gpu_backend_v3.rs");
        let forbidden_derivation = ["binding_identity", ".as_bytes()"].concat();
        assert!(!source.contains(&forbidden_derivation));
        assert!(
            !include_str!("../../fe2o3-debug-protocol/src/live_gpu_v3.rs")
                .contains("pub stopped_scope")
        );
    }

    #[test]
    fn protocol_projection_failure_poisons_the_actual_backend() {
        let mut backend = LiveKfdBackendV3::new(EmptyTransportV3, binding(), test_scope(74));
        let mut envelope = native_stopped_envelope();
        envelope.queue_identity = envelope.identity;
        let failed = convert_stopped_queue_capture(
            StoppedQueueEnvelopeCaptureV2::Captured {
                session: HardwareSessionViewV2 {
                    state: HardwareSessionStateV2::Running,
                    commands_processed: 1,
                    control_revision: 0,
                    observation_sequence: 0,
                    identity_generation: 1,
                    runtime_enabled: true,
                    hardware_observed: true,
                    simulated: false,
                    performance_prediction: false,
                },
                queue: HardwareQueueIdV2 {
                    generation: 1,
                    ordinal: 1,
                },
                device: HardwareDeviceIdV2 {
                    generation: 1,
                    ordinal: 1,
                },
                envelope,
            },
            1,
            &backend.binding,
            &mut backend.hardware,
        );
        assert!(matches!(
            failed,
            LiveGpuDebugResponseV3::Error {
                session: LiveGpuSessionViewV3 {
                    state: LiveGpuSessionStateV3::Poisoned,
                    ..
                },
                error: LiveGpuErrorV3 {
                    effect: HardwareEffectV2::Indeterminate,
                    terminal: true,
                    ..
                },
                ..
            }
        ));
        let next = backend.handle(LiveGpuDebugRequestV3::GetState {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id: 2,
            expected_revision: 0,
        });
        assert!(matches!(
            next,
            LiveGpuDebugResponseV3::Error {
                session: LiveGpuSessionViewV3 {
                    state: LiveGpuSessionStateV3::Poisoned,
                    ..
                },
                error: LiveGpuErrorV3 {
                    code: LiveGpuErrorCodeV3::SessionPoisoned,
                    terminal: true,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn running_kfd_capabilities_do_not_claim_stopped_state() {
        let mut backend = LiveKfdBackendV3::new(EmptyTransportV3, binding(), test_scope(71));
        let response = backend.handle(LiveGpuDebugRequestV3::DiscoverCapabilities {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id: 1,
            expected_revision: 0,
        });
        response.validate(backend.limits()).unwrap();
        let LiveGpuDebugResponseV3::Ok {
            result, session, ..
        } = response
        else {
            panic!("capability query failed")
        };
        assert_eq!(session.state, LiveGpuSessionStateV3::Running);
        let LiveGpuDebugResultV3::Capabilities { capabilities } = *result else {
            panic!("wrong result")
        };
        assert!(capabilities.iter().any(|capability| {
            capability.name == LiveGpuCapabilityNameV3::HardwareQueueSnapshot
                && capability.availability == LiveGpuCapabilityAvailabilityV3::Available
        }));
        assert!(capabilities.iter().any(|capability| {
            capability.name == LiveGpuCapabilityNameV3::StoppedWaves
                && capability.availability == LiveGpuCapabilityAvailabilityV3::Unavailable
        }));
    }

    #[test]
    fn semantic_query_is_typed_unavailable_after_kfd_state_refresh() {
        let mut backend = LiveKfdBackendV3::new(EmptyTransportV3, binding(), test_scope(72));
        let response = backend.handle(LiveGpuDebugRequestV3::InspectStoppedScopes {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id: 1,
            expected_revision: 0,
            binding_identity: identity(1),
            stop_identity: identity(9),
            scope: LiveGpuScopeSelectorV3::Dispatch {
                dispatch: LiveGpuDispatchIdentityV3 {
                    domain: LiveGpuDispatchIdentityDomainV3::RuntimeModel,
                    identity: identity(10),
                },
            },
            page: LiveGpuPageRequestV3 {
                snapshot_identity: identity(11),
                start: 0,
                limit: 1,
            },
        });
        response.validate(backend.limits()).unwrap();
        assert!(matches!(
            response,
            LiveGpuDebugResponseV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::Unsupported,
                ..
            }
        ));
    }

    fn telemetry_digest(seed: u8) -> KfdTargetDebugTelemetryDigestV1 {
        KfdTargetDebugTelemetryDigestV1::from_bytes([seed; 32]).unwrap()
    }

    #[test]
    fn cooperative_code_object_remains_a_matching_declaration() {
        let mut backend = LiveKfdBackendV3::new(EmptyTransportV3, binding(), test_scope(73));
        let nonce = KfdTargetDebugSessionNonceV1::from_bytes([12; 32]).unwrap();
        let started = KfdTargetDebugTelemetryRecordV1::new(
            0,
            nonce,
            KfdTargetDebugTelemetryPayloadV1::SessionStarted {
                process_instance: telemetry_digest(13),
                executable: KfdTargetDebugArtifactIdentityV1::new(telemetry_digest(14), 64)
                    .unwrap(),
            },
        )
        .unwrap();
        backend.apply_target_telemetry(&started).unwrap();
        let code_object = KfdTargetDebugTelemetryRecordV1::new(
            1,
            nonce,
            KfdTargetDebugTelemetryPayloadV1::Artifact {
                role: KfdTargetDebugArtifactRoleV1::CodeObject,
                ordinal: 0,
                artifact: KfdTargetDebugArtifactIdentityV1::new(telemetry_digest(2), 64).unwrap(),
            },
        )
        .unwrap();
        backend.apply_target_telemetry(&code_object).unwrap();
        let LiveGpuAvailabilityV3::Available { truth, .. } =
            &backend.binding.target_declared_code_object
        else {
            panic!("target code-object declaration was not retained")
        };
        assert_eq!(truth.origin, LiveGpuTruthOriginV3::Declared);
        assert!(matches!(
            &backend.binding.execution_code_object,
            LiveGpuAvailabilityV3::Unavailable { .. }
        ));
        let LiveGpuAvailabilityV3::Available { value, .. } = &backend.binding.target_telemetry
        else {
            panic!("target telemetry summary was not retained")
        };
        assert_eq!(value.records, 2);
        assert_eq!(value.artifact_records, 1);
    }
}
