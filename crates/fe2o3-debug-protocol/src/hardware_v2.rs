use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

pub const HARDWARE_REQUEST_SCHEMA_V2: &str = "fe2o3-hardware-debug-request-v2";
pub const HARDWARE_RESPONSE_SCHEMA_V2: &str = "fe2o3-hardware-debug-response-v2";
pub const MAX_HARDWARE_REQUEST_LINE_BYTES_V2: usize = 64 * 1024;
pub const MAX_HARDWARE_RESPONSE_LINE_BYTES_V2: usize = 1024 * 1024;
pub const MAX_HARDWARE_PAGE_ITEMS_V2: u16 = 256;
pub const MAX_HARDWARE_QUEUE_CONTROL_ITEMS_V2: usize = 256;
pub const MAX_HARDWARE_EVENT_WAIT_MILLISECONDS_V2: u16 = 1_000;
pub const MAX_HARDWARE_SUSPEND_GRACE_PERIOD_V2: u32 = 1_000;
pub const MAX_HARDWARE_ERROR_MESSAGE_BYTES_V2: usize = 256;
pub const MAX_HARDWARE_RETAINED_EVENTS_V2: usize = 4_096;
pub const MAX_HARDWARE_SESSION_COMMANDS_V2: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareProtocolLimitsV2 {
    pub max_request_line_bytes: usize,
    pub max_response_line_bytes: usize,
    pub max_page_items: u16,
    pub max_queue_control_items: usize,
}

impl HardwareProtocolLimitsV2 {
    pub fn validate(self) -> Result<(), HardwareProtocolValidationErrorV2> {
        if self.max_request_line_bytes == 0
            || self.max_request_line_bytes > MAX_HARDWARE_REQUEST_LINE_BYTES_V2
        {
            return Err(HardwareProtocolValidationErrorV2::LimitOutOfRange(
                "max_request_line_bytes",
            ));
        }
        if self.max_response_line_bytes == 0
            || self.max_response_line_bytes > MAX_HARDWARE_RESPONSE_LINE_BYTES_V2
        {
            return Err(HardwareProtocolValidationErrorV2::LimitOutOfRange(
                "max_response_line_bytes",
            ));
        }
        if self.max_page_items == 0 || self.max_page_items > MAX_HARDWARE_PAGE_ITEMS_V2 {
            return Err(HardwareProtocolValidationErrorV2::LimitOutOfRange(
                "max_page_items",
            ));
        }
        if self.max_queue_control_items == 0
            || self.max_queue_control_items > MAX_HARDWARE_QUEUE_CONTROL_ITEMS_V2
        {
            return Err(HardwareProtocolValidationErrorV2::LimitOutOfRange(
                "max_queue_control_items",
            ));
        }
        Ok(())
    }
}

impl Default for HardwareProtocolLimitsV2 {
    fn default() -> Self {
        Self {
            max_request_line_bytes: MAX_HARDWARE_REQUEST_LINE_BYTES_V2,
            max_response_line_bytes: MAX_HARDWARE_RESPONSE_LINE_BYTES_V2,
            max_page_items: MAX_HARDWARE_PAGE_ITEMS_V2,
            max_queue_control_items: MAX_HARDWARE_QUEUE_CONTROL_ITEMS_V2,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HardwareRequestSchemaV2 {
    #[serde(rename = "fe2o3-hardware-debug-request-v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HardwareResponseSchemaV2 {
    #[serde(rename = "fe2o3-hardware-debug-response-v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareDeviceIdV2 {
    pub generation: u64,
    pub ordinal: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareQueueIdV2 {
    pub generation: u64,
    pub ordinal: u32,
}

impl HardwareDeviceIdV2 {
    pub fn validate(self) -> Result<(), HardwareProtocolValidationErrorV2> {
        validate_logical_id(self.generation, self.ordinal, "device")
    }
}

impl HardwareQueueIdV2 {
    pub fn validate(self) -> Result<(), HardwareProtocolValidationErrorV2> {
        validate_logical_id(self.generation, self.ordinal, "queue")
    }
}

fn validate_logical_id(
    generation: u64,
    ordinal: u32,
    field: &'static str,
) -> Result<(), HardwareProtocolValidationErrorV2> {
    if generation == 0 || ordinal == 0 {
        Err(HardwareProtocolValidationErrorV2::InvalidLogicalId(field))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwarePageRequestV2 {
    /// Zero acquires the backend's current generation. Nonzero requires an
    /// exact match and rejects a stale page.
    pub expected_generation: u64,
    pub start: u32,
    pub limit: u16,
}

impl HardwarePageRequestV2 {
    pub fn validate(
        self,
        limits: HardwareProtocolLimitsV2,
    ) -> Result<(), HardwareProtocolValidationErrorV2> {
        if self.limit == 0 || self.limit > limits.max_page_items {
            return Err(HardwareProtocolValidationErrorV2::CountOutOfRange(
                "page limit",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareEventPageRequestV2 {
    pub after_sequence: u64,
    pub limit: u16,
    pub wait_milliseconds: u16,
}

impl HardwareEventPageRequestV2 {
    pub fn validate(
        self,
        limits: HardwareProtocolLimitsV2,
    ) -> Result<(), HardwareProtocolValidationErrorV2> {
        if self.limit == 0 || self.limit > limits.max_page_items {
            return Err(HardwareProtocolValidationErrorV2::CountOutOfRange(
                "event page limit",
            ));
        }
        if self.wait_milliseconds > MAX_HARDWARE_EVENT_WAIT_MILLISECONDS_V2 {
            return Err(HardwareProtocolValidationErrorV2::CountOutOfRange(
                "event wait",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum HardwareDebugRequestV2 {
    DiscoverCapabilities {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
    },
    GetState {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
    },
    InspectHardwareDevices {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
        page: HardwarePageRequestV2,
    },
    InspectHardwareQueues {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
        page: HardwarePageRequestV2,
    },
    QueryHardwareExceptionEvents {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
        page: HardwareEventPageRequestV2,
    },
    SuspendQueues {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
        queues: Vec<HardwareQueueIdV2>,
        grace_period: u32,
    },
    ResumeQueues {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
        queues: Vec<HardwareQueueIdV2>,
    },
    Terminate {
        schema: HardwareRequestSchemaV2,
        request_id: u64,
        expected_control_revision: u64,
    },
}

impl HardwareDebugRequestV2 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::GetState { request_id, .. }
            | Self::InspectHardwareDevices { request_id, .. }
            | Self::InspectHardwareQueues { request_id, .. }
            | Self::QueryHardwareExceptionEvents { request_id, .. }
            | Self::SuspendQueues { request_id, .. }
            | Self::ResumeQueues { request_id, .. }
            | Self::Terminate { request_id, .. } => *request_id,
        }
    }

    pub const fn expected_control_revision(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities {
                expected_control_revision,
                ..
            }
            | Self::GetState {
                expected_control_revision,
                ..
            }
            | Self::InspectHardwareDevices {
                expected_control_revision,
                ..
            }
            | Self::InspectHardwareQueues {
                expected_control_revision,
                ..
            }
            | Self::QueryHardwareExceptionEvents {
                expected_control_revision,
                ..
            }
            | Self::SuspendQueues {
                expected_control_revision,
                ..
            }
            | Self::ResumeQueues {
                expected_control_revision,
                ..
            }
            | Self::Terminate {
                expected_control_revision,
                ..
            } => *expected_control_revision,
        }
    }

    pub const fn operation(&self) -> HardwareDebugOperationV2 {
        match self {
            Self::DiscoverCapabilities { .. } => HardwareDebugOperationV2::DiscoverCapabilities,
            Self::GetState { .. } => HardwareDebugOperationV2::GetState,
            Self::InspectHardwareDevices { .. } => HardwareDebugOperationV2::InspectHardwareDevices,
            Self::InspectHardwareQueues { .. } => HardwareDebugOperationV2::InspectHardwareQueues,
            Self::QueryHardwareExceptionEvents { .. } => {
                HardwareDebugOperationV2::QueryHardwareExceptionEvents
            }
            Self::SuspendQueues { .. } => HardwareDebugOperationV2::SuspendQueues,
            Self::ResumeQueues { .. } => HardwareDebugOperationV2::ResumeQueues,
            Self::Terminate { .. } => HardwareDebugOperationV2::Terminate,
        }
    }

    pub fn validate(
        &self,
        limits: HardwareProtocolLimitsV2,
    ) -> Result<(), HardwareProtocolValidationErrorV2> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(HardwareProtocolValidationErrorV2::ZeroRequestId);
        }
        match self {
            Self::InspectHardwareDevices { page, .. }
            | Self::InspectHardwareQueues { page, .. } => page.validate(limits),
            Self::QueryHardwareExceptionEvents { page, .. } => page.validate(limits),
            Self::SuspendQueues {
                queues,
                grace_period,
                ..
            } => {
                if *grace_period > MAX_HARDWARE_SUSPEND_GRACE_PERIOD_V2 {
                    return Err(HardwareProtocolValidationErrorV2::CountOutOfRange(
                        "suspend grace period",
                    ));
                }
                validate_queue_list(queues, limits)
            }
            Self::ResumeQueues { queues, .. } => validate_queue_list(queues, limits),
            _ => Ok(()),
        }
    }
}

fn validate_queue_list(
    queues: &[HardwareQueueIdV2],
    limits: HardwareProtocolLimitsV2,
) -> Result<(), HardwareProtocolValidationErrorV2> {
    if queues.is_empty() || queues.len() > limits.max_queue_control_items {
        return Err(HardwareProtocolValidationErrorV2::CountOutOfRange(
            "queue control list",
        ));
    }
    let mut unique = BTreeSet::new();
    for queue in queues.iter().copied() {
        queue.validate()?;
        if !unique.insert(queue) {
            return Err(HardwareProtocolValidationErrorV2::DuplicateLogicalId(
                "queue",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareDebugOperationV2 {
    DiscoverCapabilities,
    GetState,
    InspectHardwareDevices,
    InspectHardwareQueues,
    QueryHardwareExceptionEvents,
    SuspendQueues,
    ResumeQueues,
    Terminate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareCapabilityNameV2 {
    HardwareDeviceSnapshot,
    HardwareQueueSnapshot,
    HardwareExceptionEvents,
    QueueSuspend,
    QueueResume,
    Terminate,
    WaveState,
    LaneState,
    RegisterValues,
    CwsrDecode,
    CallStack,
    SourceSites,
    KirSites,
    Step,
    Replay,
    Breakpoints,
    Values,
    TargetMemory,
    SemanticTrace,
    AddressWatch,
    DispatchSubmission,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareCapabilityAvailabilityV2 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareCapabilityV2 {
    pub name: HardwareCapabilityNameV2,
    pub availability: HardwareCapabilityAvailabilityV2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareSessionStateV2 {
    Running,
    Poisoned,
    Terminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareSessionViewV2 {
    pub state: HardwareSessionStateV2,
    pub commands_processed: u64,
    pub control_revision: u64,
    pub observation_sequence: u64,
    pub identity_generation: u64,
    pub runtime_enabled: bool,
    pub hardware_observed: bool,
    pub simulated: bool,
    pub performance_prediction: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareDeviceViewV2 {
    pub id: HardwareDeviceIdV2,
    pub gfx_target_version: u32,
    pub xcc_count: u32,
    pub trap_debug_supported: bool,
    pub debug_firmware_supported: bool,
    pub launch_mode_supported: bool,
    pub launch_override_supported: bool,
    pub precise_memory_supported: bool,
    pub precise_alu_supported: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareQueueViewV2 {
    pub id: HardwareQueueIdV2,
    pub device: HardwareDeviceIdV2,
    pub ring_bytes: u32,
    pub queue_type: u32,
    pub context_save_area_bytes: u32,
    pub suspended_by_session: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareRuntimeStateV2 {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareExceptionKindV2 {
    QueueWaveAbort,
    QueueWaveTrap,
    QueueWaveMathError,
    QueueWaveIllegalInstruction,
    QueueWaveMemoryViolation,
    QueueWaveApertureViolation,
    QueuePacketDispatchDimensionsInvalid,
    QueuePacketDispatchGroupSegmentSizeInvalid,
    QueuePacketDispatchCodeInvalid,
    QueuePacketReserved,
    QueuePacketUnsupported,
    QueuePacketDispatchWorkgroupSizeInvalid,
    QueuePacketDispatchRegisterInvalid,
    QueuePacketVendorUnsupported,
    QueuePreemptionError,
    QueueNew,
    DeviceQueueDelete,
    DeviceMemoryViolation,
    DeviceRasError,
    DeviceFatalHalt,
    DeviceNew,
    ProcessDeviceRemove,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum HardwareEventScopeV2 {
    Process,
    Device {
        device: HardwareDeviceIdV2,
    },
    Queue {
        device: HardwareDeviceIdV2,
        queue: HardwareQueueIdV2,
    },
    UnresolvedNativeSource,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HardwareEventPayloadV2 {
    RuntimeTransition {
        state: HardwareRuntimeStateV2,
    },
    Exception {
        exception: HardwareExceptionKindV2,
        scope: HardwareEventScopeV2,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareEventViewV2 {
    pub sequence: u64,
    pub identity_generation: u64,
    pub payload: HardwareEventPayloadV2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareQueueControlStateV2 {
    Complete,
    HardwareError,
    Invalid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareQueueControlResultV2 {
    pub queue: HardwareQueueIdV2,
    pub state: HardwareQueueControlStateV2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareEffectV2 {
    None,
    Committed,
    Partial,
    Indeterminate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum HardwareDebugResultV2 {
    Capabilities {
        capabilities: Vec<HardwareCapabilityV2>,
    },
    State,
    Devices {
        generation: u64,
        items: Vec<HardwareDeviceViewV2>,
        next_start: u32,
    },
    Queues {
        generation: u64,
        items: Vec<HardwareQueueViewV2>,
        next_start: u32,
    },
    Events {
        items: Vec<HardwareEventViewV2>,
        next_after_sequence: u64,
    },
    QueueControl {
        outcomes: Vec<HardwareQueueControlResultV2>,
        effect: HardwareEffectV2,
    },
    Terminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareErrorStageV2 {
    Framing,
    Validation,
    Session,
    Snapshot,
    Event,
    Control,
    Cleanup,
    Output,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareErrorCodeV2 {
    InvalidJson,
    InvalidRequest,
    StaleControlRevision,
    StaleIdentityGeneration,
    StaleEventCursor,
    RuntimeNotEnabled,
    UnknownLogicalId,
    ResourceLimit,
    BackendFailure,
    SessionPoisoned,
    SessionTerminated,
    ResponseTooLarge,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareDebugErrorV2 {
    pub stage: HardwareErrorStageV2,
    pub code: HardwareErrorCodeV2,
    pub effect: HardwareEffectV2,
    pub terminal: bool,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareUnavailableReasonV2 {
    NotProvidedByKfd,
    RuntimeNotEnabled,
    DeviceCapabilityAbsent,
    GeneralLaunchUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum HardwareDebugResponseV2 {
    Ok {
        schema: HardwareResponseSchemaV2,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        session: HardwareSessionViewV2,
        result: HardwareDebugResultV2,
    },
    Unavailable {
        schema: HardwareResponseSchemaV2,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        session: HardwareSessionViewV2,
        capability: HardwareCapabilityNameV2,
        reason: HardwareUnavailableReasonV2,
        detail: String,
    },
    Error {
        schema: HardwareResponseSchemaV2,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        session: HardwareSessionViewV2,
        error: HardwareDebugErrorV2,
    },
}

impl HardwareDebugResponseV2 {
    pub fn validate(
        &self,
        limits: HardwareProtocolLimitsV2,
    ) -> Result<(), HardwareProtocolValidationErrorV2> {
        limits.validate()?;
        let (request_id, session) = match self {
            Self::Ok {
                request_id,
                session,
                result,
                ..
            } => {
                validate_result(result, *session, limits)?;
                (*request_id, *session)
            }
            Self::Unavailable {
                request_id,
                session,
                detail,
                ..
            } => {
                validate_text(detail)?;
                (*request_id, *session)
            }
            Self::Error {
                request_id,
                session,
                error,
                ..
            } => {
                validate_text(&error.message)?;
                if error.terminal != matches!(session.state, HardwareSessionStateV2::Poisoned) {
                    return Err(HardwareProtocolValidationErrorV2::InvalidSession);
                }
                (*request_id, *session)
            }
        };
        if request_id == 0
            || session.identity_generation == 0
            || session.commands_processed == 0
            || session.commands_processed > MAX_HARDWARE_SESSION_COMMANDS_V2
            || session.simulated
            || !session.hardware_observed
            || session.performance_prediction
        {
            return Err(HardwareProtocolValidationErrorV2::InvalidSession);
        }
        Ok(())
    }
}

fn validate_result(
    result: &HardwareDebugResultV2,
    session: HardwareSessionViewV2,
    limits: HardwareProtocolLimitsV2,
) -> Result<(), HardwareProtocolValidationErrorV2> {
    if matches!(result, HardwareDebugResultV2::Terminated)
        != matches!(session.state, HardwareSessionStateV2::Terminated)
        || matches!(session.state, HardwareSessionStateV2::Poisoned)
    {
        return Err(HardwareProtocolValidationErrorV2::InvalidSession);
    }
    let count = match result {
        HardwareDebugResultV2::Capabilities { capabilities } => {
            let mut names = BTreeSet::new();
            if capabilities
                .iter()
                .any(|capability| !names.insert(capability.name))
            {
                return Err(HardwareProtocolValidationErrorV2::DuplicateCapability);
            }
            capabilities.len()
        }
        HardwareDebugResultV2::Devices {
            generation, items, ..
        } => {
            let mut ids = BTreeSet::new();
            if *generation == 0
                || *generation != session.identity_generation
                || items.iter().any(|item| {
                    item.id.validate().is_err()
                        || item.id.generation != *generation
                        || !ids.insert(item.id)
                })
            {
                return Err(HardwareProtocolValidationErrorV2::InvalidLogicalId(
                    "device page",
                ));
            }
            items.len()
        }
        HardwareDebugResultV2::Queues {
            generation, items, ..
        } => {
            let mut ids = BTreeSet::new();
            if *generation == 0
                || *generation != session.identity_generation
                || items.iter().any(|item| {
                    item.id.validate().is_err()
                        || item.device.validate().is_err()
                        || item.id.generation != *generation
                        || item.device.generation != *generation
                        || !ids.insert(item.id)
                })
            {
                return Err(HardwareProtocolValidationErrorV2::InvalidLogicalId(
                    "queue page",
                ));
            }
            items.len()
        }
        HardwareDebugResultV2::Events {
            items,
            next_after_sequence,
        } => {
            let valid = items.iter().try_fold(0_u64, |prior, event| {
                if event.sequence == 0
                    || event.sequence <= prior
                    || event.sequence > session.observation_sequence
                    || event.identity_generation == 0
                    || !valid_event_scope(*event)
                {
                    None
                } else {
                    Some(event.sequence)
                }
            });
            if valid.is_none() && !items.is_empty() {
                return Err(HardwareProtocolValidationErrorV2::InvalidEventSequence);
            }
            if items
                .last()
                .is_some_and(|event| event.sequence != *next_after_sequence)
                || *next_after_sequence > session.observation_sequence
            {
                return Err(HardwareProtocolValidationErrorV2::InvalidEventSequence);
            }
            items.len()
        }
        HardwareDebugResultV2::QueueControl { outcomes, effect } => {
            let mut ids = BTreeSet::new();
            if outcomes.iter().any(|item| {
                item.queue.validate().is_err()
                    || item.queue.generation != session.identity_generation
                    || !ids.insert(item.queue)
            }) {
                return Err(HardwareProtocolValidationErrorV2::InvalidLogicalId(
                    "queue outcome",
                ));
            }
            let complete = outcomes
                .iter()
                .filter(|item| item.state == HardwareQueueControlStateV2::Complete)
                .count();
            let valid_effect = match effect {
                HardwareEffectV2::Committed => complete == outcomes.len() && !outcomes.is_empty(),
                HardwareEffectV2::Partial => complete > 0 && complete < outcomes.len(),
                HardwareEffectV2::None => complete == 0,
                HardwareEffectV2::Indeterminate => false,
            };
            if !valid_effect {
                return Err(HardwareProtocolValidationErrorV2::InvalidSession);
            }
            outcomes.len()
        }
        HardwareDebugResultV2::State | HardwareDebugResultV2::Terminated => 0,
    };
    if count > usize::from(limits.max_page_items) {
        return Err(HardwareProtocolValidationErrorV2::CountOutOfRange(
            "response items",
        ));
    }
    Ok(())
}

fn valid_event_scope(event: HardwareEventViewV2) -> bool {
    match event.payload {
        HardwareEventPayloadV2::RuntimeTransition { .. } => true,
        HardwareEventPayloadV2::Exception { scope, .. } => match scope {
            HardwareEventScopeV2::Process | HardwareEventScopeV2::UnresolvedNativeSource => true,
            HardwareEventScopeV2::Device { device } => {
                device.validate().is_ok() && device.generation == event.identity_generation
            }
            HardwareEventScopeV2::Queue { device, queue } => {
                device.validate().is_ok()
                    && queue.validate().is_ok()
                    && device.generation == event.identity_generation
                    && queue.generation == event.identity_generation
            }
        },
    }
}

fn validate_text(text: &str) -> Result<(), HardwareProtocolValidationErrorV2> {
    if text.is_empty()
        || text.len() > MAX_HARDWARE_ERROR_MESSAGE_BYTES_V2
        || text.chars().any(char::is_control)
    {
        Err(HardwareProtocolValidationErrorV2::CountOutOfRange(
            "message",
        ))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareProtocolValidationErrorV2 {
    LimitOutOfRange(&'static str),
    ZeroRequestId,
    CountOutOfRange(&'static str),
    InvalidLogicalId(&'static str),
    DuplicateLogicalId(&'static str),
    DuplicateCapability,
    InvalidEventSequence,
    InvalidSession,
}

impl fmt::Display for HardwareProtocolValidationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "hardware debug protocol validation failed: {self:?}"
        )
    }
}

impl std::error::Error for HardwareProtocolValidationErrorV2 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareProtocolCodecErrorV2 {
    Validation(HardwareProtocolValidationErrorV2),
    EmptyLine,
    LineTooLarge,
    MissingLineTerminator,
    EmbeddedLineBreak,
    InvalidJson,
    InputRead,
    AllocationFailure,
    ResponseTooLarge,
    JsonEncode,
}

impl fmt::Display for HardwareProtocolCodecErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "hardware debug protocol codec failed: {self:?}")
    }
}

impl std::error::Error for HardwareProtocolCodecErrorV2 {}

pub fn decode_hardware_request_line_v2(
    line: &[u8],
    limits: HardwareProtocolLimitsV2,
) -> Result<HardwareDebugRequestV2, HardwareProtocolCodecErrorV2> {
    let payload = validate_line(line, limits.max_request_line_bytes)?;
    let request: HardwareDebugRequestV2 =
        serde_json::from_slice(payload).map_err(|_| HardwareProtocolCodecErrorV2::InvalidJson)?;
    request
        .validate(limits)
        .map_err(HardwareProtocolCodecErrorV2::Validation)?;
    Ok(request)
}

pub fn decode_hardware_response_line_v2(
    line: &[u8],
    limits: HardwareProtocolLimitsV2,
) -> Result<HardwareDebugResponseV2, HardwareProtocolCodecErrorV2> {
    let payload = validate_line(line, limits.max_response_line_bytes)?;
    let response: HardwareDebugResponseV2 =
        serde_json::from_slice(payload).map_err(|_| HardwareProtocolCodecErrorV2::InvalidJson)?;
    response
        .validate(limits)
        .map_err(HardwareProtocolCodecErrorV2::Validation)?;
    Ok(response)
}

pub fn read_hardware_request_line_v2<R: BufRead>(
    reader: &mut R,
    limits: HardwareProtocolLimitsV2,
) -> Result<Option<HardwareDebugRequestV2>, HardwareProtocolCodecErrorV2> {
    limits
        .validate()
        .map_err(HardwareProtocolCodecErrorV2::Validation)?;
    let mut line = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| HardwareProtocolCodecErrorV2::InputRead)?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(HardwareProtocolCodecErrorV2::MissingLineTerminator)
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |position| position + 1);
        append_bounded(
            &mut line,
            &available[..consumed],
            limits.max_request_line_bytes,
        )?;
        reader.consume(consumed);
        if newline.is_some() {
            return decode_hardware_request_line_v2(&line, limits).map(Some);
        }
    }
}

pub fn encode_hardware_response_line_v2(
    response: &HardwareDebugResponseV2,
    limits: HardwareProtocolLimitsV2,
) -> Result<Vec<u8>, HardwareProtocolCodecErrorV2> {
    response
        .validate(limits)
        .map_err(HardwareProtocolCodecErrorV2::Validation)?;
    let payload_limit = limits
        .max_response_line_bytes
        .checked_sub(1)
        .ok_or(HardwareProtocolCodecErrorV2::ResponseTooLarge)?;
    let mut output = Vec::new();
    let mut writer = BoundedWriterV2 {
        output: &mut output,
        max: payload_limit,
        limit_exceeded: false,
        allocation_failed: false,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(if writer.limit_exceeded {
            HardwareProtocolCodecErrorV2::ResponseTooLarge
        } else if writer.allocation_failed {
            HardwareProtocolCodecErrorV2::AllocationFailure
        } else {
            HardwareProtocolCodecErrorV2::JsonEncode
        });
    }
    append_bounded(&mut output, b"\n", limits.max_response_line_bytes)?;
    Ok(output)
}

fn validate_line(line: &[u8], max: usize) -> Result<&[u8], HardwareProtocolCodecErrorV2> {
    if line.is_empty() {
        return Err(HardwareProtocolCodecErrorV2::EmptyLine);
    }
    if line.len() > max {
        return Err(HardwareProtocolCodecErrorV2::LineTooLarge);
    }
    let payload = line
        .strip_suffix(b"\n")
        .ok_or(HardwareProtocolCodecErrorV2::MissingLineTerminator)?;
    if payload.is_empty() {
        return Err(HardwareProtocolCodecErrorV2::EmptyLine);
    }
    if payload.iter().any(|byte| matches!(*byte, b'\n' | b'\r')) {
        return Err(HardwareProtocolCodecErrorV2::EmbeddedLineBreak);
    }
    Ok(payload)
}

fn append_bounded(
    output: &mut Vec<u8>,
    bytes: &[u8],
    max: usize,
) -> Result<(), HardwareProtocolCodecErrorV2> {
    let required = output
        .len()
        .checked_add(bytes.len())
        .ok_or(HardwareProtocolCodecErrorV2::LineTooLarge)?;
    if required > max {
        return Err(HardwareProtocolCodecErrorV2::LineTooLarge);
    }
    if required > output.capacity() {
        output
            .try_reserve_exact(required - output.capacity())
            .map_err(|_| HardwareProtocolCodecErrorV2::AllocationFailure)?;
    }
    output.extend_from_slice(bytes);
    Ok(())
}

struct BoundedWriterV2<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    limit_exceeded: bool,
    allocation_failed: bool,
}

impl Write for BoundedWriterV2<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.limit_exceeded = true;
            return Err(io::Error::other("hardware response overflow"));
        };
        if required > self.max {
            self.limit_exceeded = true;
            return Err(io::Error::other("hardware response exceeds bound"));
        }
        if required > self.output.capacity()
            && self
                .output
                .try_reserve_exact(required - self.output.capacity())
                .is_err()
        {
            self.allocation_failed = true;
            return Err(io::Error::other("hardware response allocation failed"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
