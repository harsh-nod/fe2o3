//! Artifact-bound V3 facade over the production KFD hardware V2 state machine.

use fe2o3_debug_protocol::*;
use fe2o3_kfd::{
    KfdTargetDebugArtifactRoleV1, KfdTargetDebugTelemetryPayloadV1, KfdTargetDebugTelemetryRecordV1,
};
use sha2::{Digest, Sha256};

use crate::hardware_v2::{HardwareBackendV2, HardwareDebugTransportV2, HardwareTransportErrorV2};

pub(crate) struct LiveKfdBackendV3<T: HardwareDebugTransportV2> {
    hardware: HardwareBackendV2<T>,
    binding: LiveGpuArtifactBindingV3,
    limits: LiveGpuProtocolLimitsV3,
}

impl<T: HardwareDebugTransportV2> LiveKfdBackendV3<T> {
    pub(crate) fn new(transport: T, binding: LiveGpuArtifactBindingV3) -> Self {
        Self {
            hardware: HardwareBackendV2::new(transport),
            binding,
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
        let request_id = request.request_id();
        let operation = request.operation();
        let hardware_request = hardware_request(&request);
        convert_response(
            self.hardware.handle(hardware_request),
            request_id,
            operation,
            &self.binding,
        )
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
        | LiveGpuDebugRequestV3::InspectStoppedScopes { .. }
        | LiveGpuDebugRequestV3::InspectRegisters { .. }
        | LiveGpuDebugRequestV3::InspectValues { .. }
        | LiveGpuDebugRequestV3::ReadMemory { .. }
        | LiveGpuDebugRequestV3::ResolveProgramSite { .. } => HardwareDebugRequestV2::GetState {
            schema: HardwareRequestSchemaV2::V2,
            request_id,
            expected_control_revision,
        },
    }
}

fn convert_response(
    response: HardwareDebugResponseV2,
    request_id: u64,
    operation: LiveGpuOperationV3,
    binding: &LiveGpuArtifactBindingV3,
) -> LiveGpuDebugResponseV3 {
    match response {
        HardwareDebugResponseV2::Ok {
            session, result, ..
        } => {
            let session = live_session(session, binding.binding_identity);
            let result = match operation {
                LiveGpuOperationV3::DiscoverCapabilities => {
                    let HardwareDebugResultV2::Capabilities { capabilities } = result else {
                        return backend_protocol_error(request_id, operation, session);
                    };
                    LiveGpuDebugResultV3::Capabilities {
                        capabilities: live_capabilities(&capabilities, binding),
                    }
                }
                LiveGpuOperationV3::GetSessionBinding => {
                    if !matches!(result, HardwareDebugResultV2::State) {
                        return backend_protocol_error(request_id, operation, session);
                    }
                    LiveGpuDebugResultV3::SessionBinding {
                        binding: binding.clone(),
                    }
                }
                LiveGpuOperationV3::GetState => {
                    if !matches!(result, HardwareDebugResultV2::State) {
                        return backend_protocol_error(request_id, operation, session);
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
                        return backend_protocol_error(request_id, operation, session);
                    }
                    LiveGpuDebugResultV3::Terminated
                }
                LiveGpuOperationV3::InspectStoppedScopes
                | LiveGpuOperationV3::InspectRegisters
                | LiveGpuOperationV3::InspectValues
                | LiveGpuOperationV3::ReadMemory
                | LiveGpuOperationV3::ResolveProgramSite => {
                    return LiveGpuDebugResponseV3::Unavailable {
                        schema: LiveGpuResponseSchemaV3::V3,
                        request_id,
                        operation,
                        session,
                        reason: LiveGpuUnavailableReasonV3::Unsupported,
                    };
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
    ] {
        capabilities.push(live_capability(
            name,
            false,
            Some(LiveGpuUnavailableReasonV3::Unsupported),
        ));
    }
    capabilities
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

fn backend_protocol_error(
    request_id: u64,
    operation: LiveGpuOperationV3,
    mut session: LiveGpuSessionViewV3,
) -> LiveGpuDebugResponseV3 {
    session.state = LiveGpuSessionStateV3::Poisoned;
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
    }

    fn identity(seed: u8) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new([seed; 32]).unwrap()
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

    #[test]
    fn running_kfd_capabilities_do_not_claim_stopped_state() {
        let mut backend = LiveKfdBackendV3::new(EmptyTransportV3, binding());
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
        let mut backend = LiveKfdBackendV3::new(EmptyTransportV3, binding());
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
        let mut backend = LiveKfdBackendV3::new(EmptyTransportV3, binding());
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
