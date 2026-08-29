//! Bounded, read-only live-GPU semantic debugger protocol.
//!
//! V3 reports one stopped observation. It is not a trace and never treats an
//! unreported workgroup, wave, or lane as absent. Identities intentionally map
//! by digest bytes to `fe2o3-semantic-trace` identities; adapters perform that
//! conversion because this wire crate does not depend on the inert trace crate.

use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::{
    AllocationIdentityV1, HardwareDebugOperationV2, HardwareDebugResponseV2, HardwareDebugResultV2,
    HardwareEffectV2, HardwareEventPageRequestV2, HardwarePageRequestV2, HardwareProtocolLimitsV2,
    HardwareQueueIdV2, HardwareResponseSchemaV2, HardwareSessionStateV2, HardwareSessionViewV2,
    KirSiteV1, MAX_HARDWARE_EVENT_WAIT_MILLISECONDS_V2, MAX_HARDWARE_QUEUE_CONTROL_ITEMS_V2,
    MAX_HARDWARE_SESSION_COMMANDS_V2, MAX_HARDWARE_SUSPEND_GRACE_PERIOD_V2, OpaqueIdentityV1,
};

pub const LIVE_GPU_REQUEST_SCHEMA_V3: &str = "fe2o3-live-gpu-debug-request-v3";
pub const LIVE_GPU_RESPONSE_SCHEMA_V3: &str = "fe2o3-live-gpu-debug-response-v3";
pub const MAX_LIVE_GPU_REQUEST_LINE_BYTES_V3: usize = 64 * 1024;
pub const MAX_LIVE_GPU_RESPONSE_LINE_BYTES_V3: usize = 2 * 1024 * 1024;
pub const MAX_LIVE_GPU_PAGE_ITEMS_V3: u16 = 256;
pub const MAX_LIVE_GPU_MEMORY_BYTES_V3: u64 = 1024 * 1024;
pub const MAX_LIVE_GPU_EVIDENCE_REFS_V3: usize = 16;
pub const MAX_LIVE_GPU_SELECTOR_ITEMS_V3: usize = 256;
pub const MAX_LIVE_GPU_TEXT_BYTES_V3: usize = 128;
pub const MAX_LIVE_GPU_VALUE_BITS_V3: u16 = 4_096;
pub const MAX_LIVE_GPU_TARGET_TELEMETRY_RECORDS_V3: u64 = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGpuProtocolLimitsV3 {
    pub max_request_line_bytes: usize,
    pub max_response_line_bytes: usize,
    pub max_page_items: u16,
    pub max_memory_bytes: u64,
    pub max_evidence_refs: usize,
}

impl Default for LiveGpuProtocolLimitsV3 {
    fn default() -> Self {
        Self {
            max_request_line_bytes: MAX_LIVE_GPU_REQUEST_LINE_BYTES_V3,
            max_response_line_bytes: MAX_LIVE_GPU_RESPONSE_LINE_BYTES_V3,
            max_page_items: MAX_LIVE_GPU_PAGE_ITEMS_V3,
            max_memory_bytes: MAX_LIVE_GPU_MEMORY_BYTES_V3,
            max_evidence_refs: MAX_LIVE_GPU_EVIDENCE_REFS_V3,
        }
    }
}

impl LiveGpuProtocolLimitsV3 {
    pub fn validate(self) -> Result<(), LiveGpuValidationErrorV3> {
        if self.max_request_line_bytes == 0
            || self.max_request_line_bytes > MAX_LIVE_GPU_REQUEST_LINE_BYTES_V3
        {
            return Err(LiveGpuValidationErrorV3::LimitOutOfRange(
                "max_request_line_bytes",
            ));
        }
        if self.max_response_line_bytes == 0
            || self.max_response_line_bytes > MAX_LIVE_GPU_RESPONSE_LINE_BYTES_V3
        {
            return Err(LiveGpuValidationErrorV3::LimitOutOfRange(
                "max_response_line_bytes",
            ));
        }
        if self.max_page_items == 0 || self.max_page_items > MAX_LIVE_GPU_PAGE_ITEMS_V3 {
            return Err(LiveGpuValidationErrorV3::LimitOutOfRange("max_page_items"));
        }
        if self.max_memory_bytes == 0 || self.max_memory_bytes > MAX_LIVE_GPU_MEMORY_BYTES_V3 {
            return Err(LiveGpuValidationErrorV3::LimitOutOfRange(
                "max_memory_bytes",
            ));
        }
        if self.max_evidence_refs == 0 || self.max_evidence_refs > MAX_LIVE_GPU_EVIDENCE_REFS_V3 {
            return Err(LiveGpuValidationErrorV3::LimitOutOfRange(
                "max_evidence_refs",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LiveGpuRequestSchemaV3 {
    #[serde(rename = "fe2o3-live-gpu-debug-request-v3")]
    V3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LiveGpuResponseSchemaV3 {
    #[serde(rename = "fe2o3-live-gpu-debug-response-v3")]
    V3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuBackendV3 {
    DirectKfd,
    RocgdbMi,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuCapabilityNameV3 {
    ExactArtifactBinding,
    CooperativeTargetTelemetry,
    CpuReferenceEvidence,
    HardwareDeviceSnapshot,
    HardwareQueueSnapshot,
    HardwareExceptionEvents,
    QueueSuspend,
    QueueResume,
    Terminate,
    StoppedDispatch,
    StoppedWorkgroups,
    StoppedWaves,
    StoppedLanes,
    RelativeProgramCounter,
    IsaSite,
    KirSite,
    SourceSite,
    RegisterValues,
    SemanticValues,
    AllocationRelativeMemory,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuCapabilityAvailabilityV3 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuUnavailableReasonV3 {
    Unsupported,
    BackendNotConnected,
    SessionNotStopped,
    NotObserved,
    NotCaptured,
    OptimizedOut,
    OutsideCaptureScope,
    Truncated,
    CaptureBudgetExhausted,
    AuthenticatedBindingRequired,
    AllocationUnknown,
    PolicyRedacted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuCapabilityV3 {
    pub backend: LiveGpuBackendV3,
    pub name: LiveGpuCapabilityNameV3,
    pub availability: LiveGpuCapabilityAvailabilityV3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<LiveGpuUnavailableReasonV3>,
}

impl LiveGpuCapabilityV3 {
    fn validate(self) -> Result<(), LiveGpuValidationErrorV3> {
        if matches!(
            self.availability,
            LiveGpuCapabilityAvailabilityV3::Available
        ) == self.unavailable_reason.is_some()
        {
            return Err(LiveGpuValidationErrorV3::InvalidAvailability);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuEvidenceKindV3 {
    Declaration,
    Proof,
    InferenceRule,
    RuntimeObservation,
    Artifact,
}

/// Wire-compatible counterpart of semantic-trace `EvidenceRefV1`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuEvidenceRefV3 {
    pub kind: LiveGpuEvidenceKindV3,
    pub identity: OpaqueIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuTruthOriginV3 {
    Declared,
    Proved,
    Observed,
    Inferred,
    Unavailable,
}

/// A fact's origin and its bounded supporting evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuTruthV3 {
    pub origin: LiveGpuTruthOriginV3,
    pub evidence: Vec<LiveGpuEvidenceRefV3>,
}

impl LiveGpuTruthV3 {
    fn validate(&self, limits: LiveGpuProtocolLimitsV3) -> Result<(), LiveGpuValidationErrorV3> {
        if self.evidence.len() > limits.max_evidence_refs {
            return Err(LiveGpuValidationErrorV3::CountOutOfRange("evidence"));
        }
        let unique: BTreeSet<_> = self.evidence.iter().copied().collect();
        if unique.len() != self.evidence.len() {
            return Err(LiveGpuValidationErrorV3::DuplicateIdentity("evidence"));
        }
        let required = match self.origin {
            LiveGpuTruthOriginV3::Declared => Some(LiveGpuEvidenceKindV3::Declaration),
            LiveGpuTruthOriginV3::Proved => Some(LiveGpuEvidenceKindV3::Proof),
            LiveGpuTruthOriginV3::Observed => Some(LiveGpuEvidenceKindV3::RuntimeObservation),
            LiveGpuTruthOriginV3::Inferred => Some(LiveGpuEvidenceKindV3::InferenceRule),
            LiveGpuTruthOriginV3::Unavailable => None,
        };
        if let Some(required) = required {
            if self
                .evidence
                .iter()
                .filter(|item| item.kind == required)
                .count()
                != 1
            {
                return Err(LiveGpuValidationErrorV3::InvalidTruthEvidence);
            }
        } else if !self.evidence.is_empty() {
            return Err(LiveGpuValidationErrorV3::InvalidTruthEvidence);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuRedactionReasonV3 {
    AbsoluteTargetLocation,
    ProcessAuthority,
    BackendHandle,
    Policy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuAvailabilityV3<T> {
    Available {
        value: T,
        truth: LiveGpuTruthV3,
    },
    Redacted {
        reason: LiveGpuRedactionReasonV3,
        truth: LiveGpuTruthV3,
    },
    Unavailable {
        reason: LiveGpuUnavailableReasonV3,
        truth: LiveGpuTruthV3,
    },
}

impl<T> LiveGpuAvailabilityV3<T> {
    fn validate_truth(
        &self,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        let truth = match self {
            Self::Available { truth, .. } | Self::Redacted { truth, .. } => {
                if truth.origin == LiveGpuTruthOriginV3::Unavailable {
                    return Err(LiveGpuValidationErrorV3::InvalidAvailability);
                }
                truth
            }
            Self::Unavailable { truth, .. } => {
                if truth.origin != LiveGpuTruthOriginV3::Unavailable {
                    return Err(LiveGpuValidationErrorV3::InvalidAvailability);
                }
                truth
            }
        };
        truth.validate(limits)
    }
}

/// Exact-byte identity claim. Consumers still authenticate it against an owner.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuContentIdentityV3 {
    pub digest: OpaqueIdentityV1,
    pub canonical_bytes: u64,
}

impl LiveGpuContentIdentityV3 {
    fn validate(self) -> Result<(), LiveGpuValidationErrorV3> {
        if self.canonical_bytes == 0 {
            return Err(LiveGpuValidationErrorV3::ZeroCount("canonical bytes"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuCpuReferenceEvidenceV3 {
    Available {
        /// Identity of deterministic simulator evidence, never hardware evidence.
        simulator_capture_identity: OpaqueIdentityV1,
    },
    Unavailable {
        reason: LiveGpuUnavailableReasonV3,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuCpuReferenceBindingV3 {
    pub bundle_identity: OpaqueIdentityV1,
    pub request_identity: OpaqueIdentityV1,
    pub configuration_identity: OpaqueIdentityV1,
    pub deterministic_evidence: LiveGpuCpuReferenceEvidenceV3,
}

/// Bounded cooperative target declarations. These counts are not KFD observations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuTargetTelemetrySummaryV3 {
    pub records: u64,
    pub artifact_records: u32,
    pub dispatch_records: u32,
    pub allocation_records: u32,
    pub diagnostic_records: u32,
    pub session_ended: bool,
}

impl LiveGpuTargetTelemetrySummaryV3 {
    fn validate(&self, _limits: LiveGpuProtocolLimitsV3) -> Result<(), LiveGpuValidationErrorV3> {
        if self.records == 0 || self.records > MAX_LIVE_GPU_TARGET_TELEMETRY_RECORDS_V3 {
            return Err(LiveGpuValidationErrorV3::CountOutOfRange(
                "target telemetry records",
            ));
        }
        let classified = u64::from(self.artifact_records)
            .checked_add(u64::from(self.dispatch_records))
            .and_then(|count| count.checked_add(u64::from(self.allocation_records)))
            .and_then(|count| count.checked_add(u64::from(self.diagnostic_records)))
            .ok_or(LiveGpuValidationErrorV3::RangeOverflow(
                "target telemetry records",
            ))?;
        // SessionStarted and optional SessionEnded are the only unclassified records.
        let lifecycle_records = 1_u64 + u64::from(self.session_ended);
        if classified.checked_add(lifecycle_records) != Some(self.records) {
            return Err(LiveGpuValidationErrorV3::InvalidRange(
                "target telemetry record classes",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuArtifactBindingV3 {
    pub binding_identity: OpaqueIdentityV1,
    pub code_object_version: u16,
    pub declared_code_object: LiveGpuContentIdentityV3,
    pub declaration: LiveGpuTruthV3,
    pub target_declared_code_object: LiveGpuAvailabilityV3<LiveGpuContentIdentityV3>,
    pub target_telemetry: LiveGpuAvailabilityV3<LiveGpuTargetTelemetrySummaryV3>,
    pub execution_code_object: LiveGpuAvailabilityV3<LiveGpuContentIdentityV3>,
    pub kernel_ir_v7: LiveGpuContentIdentityV3,
    pub source_map_v2: LiveGpuContentIdentityV3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isa_map_v1: Option<LiveGpuContentIdentityV3>,
    pub cpu_reference: LiveGpuCpuReferenceBindingV3,
}

impl LiveGpuArtifactBindingV3 {
    pub fn validate(
        &self,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        if self.code_object_version == 0 {
            return Err(LiveGpuValidationErrorV3::ZeroCount("code object version"));
        }
        self.declared_code_object.validate()?;
        self.declaration.validate(limits)?;
        if self.declaration.origin != LiveGpuTruthOriginV3::Declared {
            return Err(LiveGpuValidationErrorV3::InvalidTruthEvidence);
        }
        self.target_declared_code_object.validate_truth(limits)?;
        if let LiveGpuAvailabilityV3::Available { value, truth } = &self.target_declared_code_object
        {
            value.validate()?;
            if truth.origin != LiveGpuTruthOriginV3::Declared || *value != self.declared_code_object
            {
                return Err(LiveGpuValidationErrorV3::IdentityMismatch(
                    "target-declared code object",
                ));
            }
        }
        self.target_telemetry.validate_truth(limits)?;
        if let LiveGpuAvailabilityV3::Available { value, truth } = &self.target_telemetry {
            value.validate(limits)?;
            if truth.origin != LiveGpuTruthOriginV3::Declared {
                return Err(LiveGpuValidationErrorV3::InvalidTruthEvidence);
            }
        }
        self.execution_code_object.validate_truth(limits)?;
        if let LiveGpuAvailabilityV3::Available { value, truth } = &self.execution_code_object {
            value.validate()?;
            if truth.origin != LiveGpuTruthOriginV3::Observed || *value != self.declared_code_object
            {
                return Err(LiveGpuValidationErrorV3::IdentityMismatch(
                    "execution code object",
                ));
            }
        }
        self.kernel_ir_v7.validate()?;
        self.source_map_v2.validate()?;
        if let Some(isa_map) = self.isa_map_v1 {
            isa_map.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuDispatchIdentityDomainV3 {
    RuntimeModel,
    ImportedCollector,
}

/// Converts to semantic-trace `DispatchIdentityV1` by domain and identity bytes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuDispatchIdentityV3 {
    pub domain: LiveGpuDispatchIdentityDomainV3,
    pub identity: OpaqueIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "level", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuScopeSelectorV3 {
    Dispatch {
        dispatch: LiveGpuDispatchIdentityV3,
    },
    Workgroup {
        dispatch: LiveGpuDispatchIdentityV3,
        workgroup: [u32; 3],
    },
    Wave {
        dispatch: LiveGpuDispatchIdentityV3,
        workgroup: [u32; 3],
        wave: u32,
    },
    Lane {
        dispatch: LiveGpuDispatchIdentityV3,
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
    },
}

impl LiveGpuScopeSelectorV3 {
    const fn dispatch(self) -> LiveGpuDispatchIdentityV3 {
        match self {
            Self::Dispatch { dispatch }
            | Self::Workgroup { dispatch, .. }
            | Self::Wave { dispatch, .. }
            | Self::Lane { dispatch, .. } => dispatch,
        }
    }

    fn validate(self) -> Result<(), LiveGpuValidationErrorV3> {
        if matches!(self, Self::Lane { lane, .. } if lane >= 64) {
            return Err(LiveGpuValidationErrorV3::CountOutOfRange("lane"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuPageRequestV3 {
    pub snapshot_identity: OpaqueIdentityV1,
    pub start: u32,
    pub limit: u16,
}

impl LiveGpuPageRequestV3 {
    fn validate(self, limits: LiveGpuProtocolLimitsV3) -> Result<(), LiveGpuValidationErrorV3> {
        if self.limit == 0 || self.limit > limits.max_page_items {
            return Err(LiveGpuValidationErrorV3::CountOutOfRange("page limit"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuPageViewV3 {
    pub snapshot_identity: OpaqueIdentityV1,
    pub start: u32,
    pub limit: u16,
    pub returned: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_start: Option<u32>,
}

impl LiveGpuPageViewV3 {
    fn validate(
        self,
        item_count: usize,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        if self.limit == 0
            || self.limit > limits.max_page_items
            || self.returned > self.limit
            || usize::from(self.returned) != item_count
        {
            return Err(LiveGpuValidationErrorV3::CountOutOfRange("page"));
        }
        if let Some(next) = self.next_start {
            let expected = self
                .start
                .checked_add(u32::from(self.returned))
                .ok_or(LiveGpuValidationErrorV3::RangeOverflow("page"))?;
            if self.returned == 0 || next != expected {
                return Err(LiveGpuValidationErrorV3::InvalidRange("next page"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuStoppedCoverageV3 {
    /// Only returned observations are known; omitted scopes may still exist.
    ObservedSubset,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuStopReasonV3 {
    Breakpoint,
    Trap,
    Exception,
    UserPause,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuStoppedAnchorV3 {
    pub snapshot_identity: OpaqueIdentityV1,
    pub stop_identity: OpaqueIdentityV1,
    pub observation_sequence: u64,
    pub binding: LiveGpuArtifactBindingV3,
    pub dispatch: LiveGpuDispatchIdentityV3,
    pub queue: HardwareQueueIdV2,
    pub reason: LiveGpuStopReasonV3,
    pub truth: LiveGpuTruthV3,
}

impl LiveGpuStoppedAnchorV3 {
    fn validate(&self, limits: LiveGpuProtocolLimitsV3) -> Result<(), LiveGpuValidationErrorV3> {
        if self.observation_sequence == 0 {
            return Err(LiveGpuValidationErrorV3::ZeroCount("observation sequence"));
        }
        self.binding.validate(limits)?;
        self.queue
            .validate()
            .map_err(|_| LiveGpuValidationErrorV3::InvalidLogicalIdentity("queue"))?;
        self.truth.validate(limits)?;
        if self.truth.origin != LiveGpuTruthOriginV3::Observed {
            return Err(LiveGpuValidationErrorV3::InvalidTruthEvidence);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "level", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuStoppedScopeV3 {
    Dispatch {
        dispatch: LiveGpuDispatchIdentityV3,
        truth: LiveGpuTruthV3,
    },
    Workgroup {
        dispatch: LiveGpuDispatchIdentityV3,
        workgroup: [u32; 3],
        truth: LiveGpuTruthV3,
    },
    Wave {
        dispatch: LiveGpuDispatchIdentityV3,
        workgroup: [u32; 3],
        wave: u32,
        wave_width: u16,
        active_mask: LiveGpuAvailabilityV3<u64>,
        truth: LiveGpuTruthV3,
    },
    Lane {
        dispatch: LiveGpuDispatchIdentityV3,
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
        wave_width: u16,
        active: LiveGpuAvailabilityV3<bool>,
        logical_workitem: LiveGpuAvailabilityV3<[u64; 3]>,
        truth: LiveGpuTruthV3,
    },
}

impl LiveGpuStoppedScopeV3 {
    fn validate(
        &self,
        anchor: &LiveGpuStoppedAnchorV3,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        let (dispatch, truth) = match self {
            Self::Dispatch { dispatch, truth }
            | Self::Workgroup {
                dispatch, truth, ..
            } => (*dispatch, truth),
            Self::Wave {
                dispatch,
                wave_width,
                active_mask,
                truth,
                ..
            } => {
                validate_wave_width(*wave_width)?;
                active_mask.validate_truth(limits)?;
                if let LiveGpuAvailabilityV3::Available { value, .. } = active_mask
                    && *wave_width == 32
                    && *value > u64::from(u32::MAX)
                {
                    return Err(LiveGpuValidationErrorV3::InvalidActiveMask);
                }
                (*dispatch, truth)
            }
            Self::Lane {
                dispatch,
                lane,
                wave_width,
                active,
                logical_workitem,
                truth,
                ..
            } => {
                validate_wave_width(*wave_width)?;
                if *lane >= *wave_width {
                    return Err(LiveGpuValidationErrorV3::CountOutOfRange("lane"));
                }
                active.validate_truth(limits)?;
                logical_workitem.validate_truth(limits)?;
                (*dispatch, truth)
            }
        };
        if dispatch != anchor.dispatch {
            return Err(LiveGpuValidationErrorV3::IdentityMismatch("dispatch"));
        }
        truth.validate(limits)?;
        if truth.origin != LiveGpuTruthOriginV3::Observed {
            return Err(LiveGpuValidationErrorV3::InvalidTruthEvidence);
        }
        Ok(())
    }
}

fn validate_wave_width(width: u16) -> Result<(), LiveGpuValidationErrorV3> {
    if matches!(width, 32 | 64) {
        Ok(())
    } else {
        Err(LiveGpuValidationErrorV3::CountOutOfRange("wave width"))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuRegisterClassV3 {
    Scalar,
    Vector,
    Predicate,
    Special,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuValueKindV3 {
    Boolean,
    SignedInteger,
    UnsignedInteger,
    FloatingPoint,
    AllocationRelativePointer,
    Bytes,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "encoding", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuValueEncodingV3 {
    Bits {
        bit_width: u16,
        bits: String,
    },
    AllocationRelative {
        allocation: AllocationIdentityV1,
        byte_offset: u64,
    },
    Bytes {
        bytes: String,
    },
}

impl LiveGpuValueEncodingV3 {
    fn validate(&self) -> Result<(), LiveGpuValidationErrorV3> {
        match self {
            Self::Bits { bit_width, bits } => validate_bits(*bit_width, bits),
            Self::AllocationRelative { allocation, .. } => validate_allocation(*allocation),
            Self::Bytes { bytes } => validate_hex_bytes(bytes, MAX_LIVE_GPU_MEMORY_BYTES_V3),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuRegisterValueV3 {
    pub register_identity: OpaqueIdentityV1,
    pub name: String,
    pub class: LiveGpuRegisterClassV3,
    pub kind: LiveGpuValueKindV3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<u16>,
    pub value: LiveGpuAvailabilityV3<LiveGpuValueEncodingV3>,
}

impl LiveGpuRegisterValueV3 {
    fn validate(&self, limits: LiveGpuProtocolLimitsV3) -> Result<(), LiveGpuValidationErrorV3> {
        validate_text(&self.name, "register name")?;
        if self.lane.is_some_and(|lane| lane >= 64) {
            return Err(LiveGpuValidationErrorV3::CountOutOfRange("register lane"));
        }
        self.value.validate_truth(limits)?;
        if let LiveGpuAvailabilityV3::Available { value, .. } = &self.value {
            value.validate()?;
            validate_value_encoding(self.kind, value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuSemanticValueV3 {
    pub value_identity: OpaqueIdentityV1,
    pub name: String,
    pub kind: LiveGpuValueKindV3,
    pub value: LiveGpuAvailabilityV3<LiveGpuValueEncodingV3>,
}

impl LiveGpuSemanticValueV3 {
    fn validate(&self, limits: LiveGpuProtocolLimitsV3) -> Result<(), LiveGpuValidationErrorV3> {
        validate_text(&self.name, "value name")?;
        self.value.validate_truth(limits)?;
        if let LiveGpuAvailabilityV3::Available { value, .. } = &self.value {
            value.validate()?;
            validate_value_encoding(self.kind, value)?;
        }
        Ok(())
    }
}

fn validate_value_encoding(
    kind: LiveGpuValueKindV3,
    value: &LiveGpuValueEncodingV3,
) -> Result<(), LiveGpuValidationErrorV3> {
    let valid = match kind {
        LiveGpuValueKindV3::Boolean
        | LiveGpuValueKindV3::SignedInteger
        | LiveGpuValueKindV3::UnsignedInteger
        | LiveGpuValueKindV3::FloatingPoint => {
            matches!(value, LiveGpuValueEncodingV3::Bits { .. })
        }
        LiveGpuValueKindV3::AllocationRelativePointer => {
            matches!(value, LiveGpuValueEncodingV3::AllocationRelative { .. })
        }
        LiveGpuValueKindV3::Bytes => matches!(value, LiveGpuValueEncodingV3::Bytes { .. }),
    };
    if valid {
        Ok(())
    } else {
        Err(LiveGpuValidationErrorV3::ValueEncodingMismatch)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuMemorySpaceV3 {
    Private,
    Workgroup,
    Global,
    Constant,
    Generic,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuMemoryBytesV3 {
    pub space: LiveGpuMemorySpaceV3,
    pub bytes: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuMemoryReadV3 {
    pub allocation: AllocationIdentityV1,
    pub byte_offset: u64,
    pub requested_bytes: u64,
    pub returned_bytes: u64,
    pub value: LiveGpuAvailabilityV3<LiveGpuMemoryBytesV3>,
}

impl LiveGpuMemoryReadV3 {
    fn validate(&self, limits: LiveGpuProtocolLimitsV3) -> Result<(), LiveGpuValidationErrorV3> {
        validate_allocation(self.allocation)?;
        if self.requested_bytes == 0
            || self.requested_bytes > limits.max_memory_bytes
            || self.returned_bytes > self.requested_bytes
            || self.byte_offset.checked_add(self.returned_bytes).is_none()
        {
            return Err(LiveGpuValidationErrorV3::InvalidRange("memory read"));
        }
        self.value.validate_truth(limits)?;
        match &self.value {
            LiveGpuAvailabilityV3::Available { value, .. } => {
                validate_hex_bytes(&value.bytes, limits.max_memory_bytes)?;
                if value.bytes.len() / 2
                    != usize::try_from(self.returned_bytes).unwrap_or(usize::MAX)
                {
                    return Err(LiveGpuValidationErrorV3::InvalidRange("memory bytes"));
                }
            }
            LiveGpuAvailabilityV3::Redacted { .. } | LiveGpuAvailabilityV3::Unavailable { .. }
                if self.returned_bytes != 0 =>
            {
                return Err(LiveGpuValidationErrorV3::UnavailableCarriesValue);
            }
            LiveGpuAvailabilityV3::Redacted { .. } | LiveGpuAvailabilityV3::Unavailable { .. } => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuRelativePcV3 {
    /// Byte offset from the selected kernel entry, never an absolute location.
    pub kernel_entry_byte_offset: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuIsaSiteV3 {
    pub instruction_ordinal: u64,
    pub kernel_entry_byte_offset: u64,
    pub instruction_bytes: u16,
}

impl LiveGpuIsaSiteV3 {
    fn validate(self) -> Result<(), LiveGpuValidationErrorV3> {
        if self.instruction_bytes == 0 {
            return Err(LiveGpuValidationErrorV3::ZeroCount("instruction bytes"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuSourceSpanV3 {
    pub source_map_identity: OpaqueIdentityV1,
    pub file_identity: OpaqueIdentityV1,
    pub byte_start: u64,
    pub byte_end: u64,
}

impl LiveGpuSourceSpanV3 {
    fn validate(self, binding: &LiveGpuArtifactBindingV3) -> Result<(), LiveGpuValidationErrorV3> {
        if self.source_map_identity != binding.source_map_v2.digest {
            return Err(LiveGpuValidationErrorV3::IdentityMismatch("source map"));
        }
        if self.byte_start >= self.byte_end {
            return Err(LiveGpuValidationErrorV3::InvalidRange("source span"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuProgramSiteV3 {
    pub relative_pc: LiveGpuAvailabilityV3<LiveGpuRelativePcV3>,
    pub isa: LiveGpuAvailabilityV3<LiveGpuIsaSiteV3>,
    pub kir: LiveGpuAvailabilityV3<KirSiteV1>,
    pub source: LiveGpuAvailabilityV3<LiveGpuSourceSpanV3>,
}

impl LiveGpuProgramSiteV3 {
    fn validate(
        &self,
        binding: &LiveGpuArtifactBindingV3,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        self.relative_pc.validate_truth(limits)?;
        self.isa.validate_truth(limits)?;
        self.kir.validate_truth(limits)?;
        self.source.validate_truth(limits)?;
        if let LiveGpuAvailabilityV3::Available { value, .. } = self.isa {
            value.validate()?;
        }
        if let LiveGpuAvailabilityV3::Available { value, .. } = self.source {
            value.validate(binding)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "selector", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuRegisterSelectorV3 {
    All,
    Names { names: Vec<String> },
}

impl LiveGpuRegisterSelectorV3 {
    fn validate(&self) -> Result<(), LiveGpuValidationErrorV3> {
        match self {
            Self::All => Ok(()),
            Self::Names { names } => validate_unique_text_list(names, "register selector"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "selector", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuValueSelectorV3 {
    All,
    Identities { identities: Vec<OpaqueIdentityV1> },
}

impl LiveGpuValueSelectorV3 {
    fn validate(&self) -> Result<(), LiveGpuValidationErrorV3> {
        match self {
            Self::All => Ok(()),
            Self::Identities { identities } => {
                if identities.is_empty() || identities.len() > MAX_LIVE_GPU_SELECTOR_ITEMS_V3 {
                    return Err(LiveGpuValidationErrorV3::CountOutOfRange("value selector"));
                }
                let unique: BTreeSet<_> = identities.iter().copied().collect();
                if unique.len() != identities.len() {
                    return Err(LiveGpuValidationErrorV3::DuplicateIdentity(
                        "value selector",
                    ));
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuOperationV3 {
    DiscoverCapabilities,
    GetSessionBinding,
    GetState,
    InspectHardwareDevices,
    InspectHardwareQueues,
    QueryHardwareExceptionEvents,
    SuspendQueues,
    ResumeQueues,
    InspectStoppedScopes,
    InspectRegisters,
    InspectValues,
    ReadMemory,
    ResolveProgramSite,
    Terminate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuDebugRequestV3 {
    DiscoverCapabilities {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
    },
    GetSessionBinding {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
    },
    GetState {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
    },
    InspectHardwareDevices {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        page: HardwarePageRequestV2,
    },
    InspectHardwareQueues {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        page: HardwarePageRequestV2,
    },
    QueryHardwareExceptionEvents {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        page: HardwareEventPageRequestV2,
    },
    SuspendQueues {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        queues: Vec<HardwareQueueIdV2>,
        grace_period: u32,
    },
    ResumeQueues {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        queues: Vec<HardwareQueueIdV2>,
    },
    InspectStoppedScopes {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        binding_identity: OpaqueIdentityV1,
        stop_identity: OpaqueIdentityV1,
        scope: LiveGpuScopeSelectorV3,
        page: LiveGpuPageRequestV3,
    },
    InspectRegisters {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        binding_identity: OpaqueIdentityV1,
        stop_identity: OpaqueIdentityV1,
        scope: LiveGpuScopeSelectorV3,
        selector: LiveGpuRegisterSelectorV3,
        page: LiveGpuPageRequestV3,
    },
    InspectValues {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        binding_identity: OpaqueIdentityV1,
        stop_identity: OpaqueIdentityV1,
        scope: LiveGpuScopeSelectorV3,
        selector: LiveGpuValueSelectorV3,
        page: LiveGpuPageRequestV3,
    },
    ReadMemory {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        binding_identity: OpaqueIdentityV1,
        stop_identity: OpaqueIdentityV1,
        scope: LiveGpuScopeSelectorV3,
        allocation: AllocationIdentityV1,
        byte_offset: u64,
        byte_len: u64,
    },
    ResolveProgramSite {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
        binding_identity: OpaqueIdentityV1,
        stop_identity: OpaqueIdentityV1,
        scope: LiveGpuScopeSelectorV3,
    },
    Terminate {
        schema: LiveGpuRequestSchemaV3,
        request_id: u64,
        expected_revision: u64,
    },
}

impl LiveGpuDebugRequestV3 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::GetSessionBinding { request_id, .. }
            | Self::GetState { request_id, .. }
            | Self::InspectHardwareDevices { request_id, .. }
            | Self::InspectHardwareQueues { request_id, .. }
            | Self::QueryHardwareExceptionEvents { request_id, .. }
            | Self::SuspendQueues { request_id, .. }
            | Self::ResumeQueues { request_id, .. }
            | Self::InspectStoppedScopes { request_id, .. }
            | Self::InspectRegisters { request_id, .. }
            | Self::InspectValues { request_id, .. }
            | Self::ReadMemory { request_id, .. }
            | Self::ResolveProgramSite { request_id, .. }
            | Self::Terminate { request_id, .. } => *request_id,
        }
    }

    pub const fn expected_revision(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities {
                expected_revision, ..
            }
            | Self::GetSessionBinding {
                expected_revision, ..
            }
            | Self::GetState {
                expected_revision, ..
            }
            | Self::InspectHardwareDevices {
                expected_revision, ..
            }
            | Self::InspectHardwareQueues {
                expected_revision, ..
            }
            | Self::QueryHardwareExceptionEvents {
                expected_revision, ..
            }
            | Self::SuspendQueues {
                expected_revision, ..
            }
            | Self::ResumeQueues {
                expected_revision, ..
            }
            | Self::InspectStoppedScopes {
                expected_revision, ..
            }
            | Self::InspectRegisters {
                expected_revision, ..
            }
            | Self::InspectValues {
                expected_revision, ..
            }
            | Self::ReadMemory {
                expected_revision, ..
            }
            | Self::ResolveProgramSite {
                expected_revision, ..
            }
            | Self::Terminate {
                expected_revision, ..
            } => *expected_revision,
        }
    }

    pub const fn operation(&self) -> LiveGpuOperationV3 {
        match self {
            Self::DiscoverCapabilities { .. } => LiveGpuOperationV3::DiscoverCapabilities,
            Self::GetSessionBinding { .. } => LiveGpuOperationV3::GetSessionBinding,
            Self::GetState { .. } => LiveGpuOperationV3::GetState,
            Self::InspectHardwareDevices { .. } => LiveGpuOperationV3::InspectHardwareDevices,
            Self::InspectHardwareQueues { .. } => LiveGpuOperationV3::InspectHardwareQueues,
            Self::QueryHardwareExceptionEvents { .. } => {
                LiveGpuOperationV3::QueryHardwareExceptionEvents
            }
            Self::SuspendQueues { .. } => LiveGpuOperationV3::SuspendQueues,
            Self::ResumeQueues { .. } => LiveGpuOperationV3::ResumeQueues,
            Self::InspectStoppedScopes { .. } => LiveGpuOperationV3::InspectStoppedScopes,
            Self::InspectRegisters { .. } => LiveGpuOperationV3::InspectRegisters,
            Self::InspectValues { .. } => LiveGpuOperationV3::InspectValues,
            Self::ReadMemory { .. } => LiveGpuOperationV3::ReadMemory,
            Self::ResolveProgramSite { .. } => LiveGpuOperationV3::ResolveProgramSite,
            Self::Terminate { .. } => LiveGpuOperationV3::Terminate,
        }
    }

    pub fn validate(
        &self,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(LiveGpuValidationErrorV3::ZeroRequestId);
        }
        match self {
            Self::InspectHardwareDevices { page, .. }
            | Self::InspectHardwareQueues { page, .. } => validate_hardware_page(*page, limits),
            Self::QueryHardwareExceptionEvents { page, .. } => {
                validate_hardware_event_page(*page, limits)
            }
            Self::SuspendQueues {
                queues,
                grace_period,
                ..
            } => {
                if *grace_period > MAX_HARDWARE_SUSPEND_GRACE_PERIOD_V2 {
                    return Err(LiveGpuValidationErrorV3::CountOutOfRange(
                        "suspend grace period",
                    ));
                }
                validate_hardware_queues(queues)
            }
            Self::ResumeQueues { queues, .. } => validate_hardware_queues(queues),
            Self::InspectStoppedScopes { scope, page, .. } => {
                scope.validate()?;
                page.validate(limits)
            }
            Self::InspectRegisters {
                scope,
                selector,
                page,
                ..
            } => {
                scope.validate()?;
                selector.validate()?;
                page.validate(limits)
            }
            Self::InspectValues {
                scope,
                selector,
                page,
                ..
            } => {
                scope.validate()?;
                selector.validate()?;
                page.validate(limits)
            }
            Self::ReadMemory {
                scope,
                allocation,
                byte_offset,
                byte_len,
                ..
            } => {
                scope.validate()?;
                validate_allocation(*allocation)?;
                if *byte_len == 0
                    || *byte_len > limits.max_memory_bytes
                    || byte_offset.checked_add(*byte_len).is_none()
                {
                    return Err(LiveGpuValidationErrorV3::InvalidRange("memory request"));
                }
                Ok(())
            }
            Self::ResolveProgramSite { scope, .. } => scope.validate(),
            Self::DiscoverCapabilities { .. }
            | Self::GetSessionBinding { .. }
            | Self::GetState { .. }
            | Self::Terminate { .. } => Ok(()),
        }
    }
}

fn validate_hardware_page(
    page: HardwarePageRequestV2,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<(), LiveGpuValidationErrorV3> {
    if page.limit == 0 || page.limit > limits.max_page_items {
        Err(LiveGpuValidationErrorV3::CountOutOfRange("hardware page"))
    } else {
        Ok(())
    }
}

fn validate_hardware_event_page(
    page: HardwareEventPageRequestV2,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<(), LiveGpuValidationErrorV3> {
    if page.limit == 0
        || page.limit > limits.max_page_items
        || page.wait_milliseconds > MAX_HARDWARE_EVENT_WAIT_MILLISECONDS_V2
    {
        Err(LiveGpuValidationErrorV3::CountOutOfRange(
            "hardware event page",
        ))
    } else {
        Ok(())
    }
}

fn validate_hardware_queues(queues: &[HardwareQueueIdV2]) -> Result<(), LiveGpuValidationErrorV3> {
    if queues.is_empty() || queues.len() > MAX_HARDWARE_QUEUE_CONTROL_ITEMS_V2 {
        return Err(LiveGpuValidationErrorV3::CountOutOfRange(
            "queue control list",
        ));
    }
    let mut unique = BTreeSet::new();
    for queue in queues.iter().copied() {
        queue
            .validate()
            .map_err(|_| LiveGpuValidationErrorV3::InvalidLogicalIdentity("queue"))?;
        if !unique.insert(queue) {
            return Err(LiveGpuValidationErrorV3::DuplicateIdentity("queue"));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuSessionStateV3 {
    Running,
    Stopped,
    Terminated,
    Poisoned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuSessionViewV3 {
    pub backend: LiveGpuBackendV3,
    pub state: LiveGpuSessionStateV3,
    pub revision: u64,
    pub commands_processed: u64,
    pub observation_sequence: u64,
    pub identity_generation: u64,
    pub runtime_enabled: bool,
    pub binding_identity: OpaqueIdentityV1,
}

impl LiveGpuSessionViewV3 {
    fn validate(self) -> Result<(), LiveGpuValidationErrorV3> {
        if self.commands_processed == 0
            || self.commands_processed > MAX_HARDWARE_SESSION_COMMANDS_V2
            || self.identity_generation == 0
        {
            return Err(LiveGpuValidationErrorV3::ZeroCount("commands processed"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuDebugResultV3 {
    Capabilities {
        capabilities: Vec<LiveGpuCapabilityV3>,
    },
    SessionBinding {
        binding: LiveGpuArtifactBindingV3,
    },
    State {
        stopped: LiveGpuAvailabilityV3<Box<LiveGpuStoppedAnchorV3>>,
    },
    Hardware {
        hardware: HardwareDebugResultV2,
    },
    Scopes {
        anchor: LiveGpuStoppedAnchorV3,
        coverage: LiveGpuStoppedCoverageV3,
        page: LiveGpuPageViewV3,
        items: Vec<LiveGpuStoppedScopeV3>,
    },
    Registers {
        anchor: LiveGpuStoppedAnchorV3,
        scope: LiveGpuScopeSelectorV3,
        page: LiveGpuPageViewV3,
        items: Vec<LiveGpuRegisterValueV3>,
    },
    Values {
        anchor: LiveGpuStoppedAnchorV3,
        scope: LiveGpuScopeSelectorV3,
        page: LiveGpuPageViewV3,
        items: Vec<LiveGpuSemanticValueV3>,
    },
    Memory {
        anchor: LiveGpuStoppedAnchorV3,
        scope: LiveGpuScopeSelectorV3,
        memory: LiveGpuMemoryReadV3,
    },
    ProgramSite {
        anchor: LiveGpuStoppedAnchorV3,
        scope: LiveGpuScopeSelectorV3,
        site: LiveGpuProgramSiteV3,
    },
    Terminated,
}

impl LiveGpuDebugResultV3 {
    fn matches_operation(&self, operation: LiveGpuOperationV3) -> bool {
        matches!(
            (self, operation),
            (
                Self::Capabilities { .. },
                LiveGpuOperationV3::DiscoverCapabilities
            ) | (
                Self::SessionBinding { .. },
                LiveGpuOperationV3::GetSessionBinding
            ) | (Self::State { .. }, LiveGpuOperationV3::GetState)
                | (
                    Self::Hardware {
                        hardware: HardwareDebugResultV2::Devices { .. }
                    },
                    LiveGpuOperationV3::InspectHardwareDevices
                )
                | (
                    Self::Hardware {
                        hardware: HardwareDebugResultV2::Queues { .. }
                    },
                    LiveGpuOperationV3::InspectHardwareQueues
                )
                | (
                    Self::Hardware {
                        hardware: HardwareDebugResultV2::Events { .. }
                    },
                    LiveGpuOperationV3::QueryHardwareExceptionEvents
                )
                | (
                    Self::Hardware {
                        hardware: HardwareDebugResultV2::QueueControl { .. }
                    },
                    LiveGpuOperationV3::SuspendQueues | LiveGpuOperationV3::ResumeQueues
                )
                | (Self::Terminated, LiveGpuOperationV3::Terminate)
                | (
                    Self::Scopes { .. },
                    LiveGpuOperationV3::InspectStoppedScopes
                )
                | (Self::Registers { .. }, LiveGpuOperationV3::InspectRegisters)
                | (Self::Values { .. }, LiveGpuOperationV3::InspectValues)
                | (Self::Memory { .. }, LiveGpuOperationV3::ReadMemory)
                | (
                    Self::ProgramSite { .. },
                    LiveGpuOperationV3::ResolveProgramSite
                )
        )
    }

    fn validate(
        &self,
        session: LiveGpuSessionViewV3,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        match self {
            Self::Capabilities { capabilities } => {
                if capabilities.len() > usize::from(limits.max_page_items) {
                    return Err(LiveGpuValidationErrorV3::CountOutOfRange("capabilities"));
                }
                let mut unique = BTreeSet::new();
                for capability in capabilities.iter().copied() {
                    capability.validate()?;
                    if !unique.insert((capability.backend, capability.name)) {
                        return Err(LiveGpuValidationErrorV3::DuplicateIdentity("capability"));
                    }
                }
                Ok(())
            }
            Self::SessionBinding { binding } => {
                binding.validate(limits)?;
                if binding.binding_identity != session.binding_identity {
                    return Err(LiveGpuValidationErrorV3::IdentityMismatch("binding"));
                }
                Ok(())
            }
            Self::State { stopped } => {
                stopped.validate_truth(limits)?;
                match (session.state, stopped) {
                    (
                        LiveGpuSessionStateV3::Stopped,
                        LiveGpuAvailabilityV3::Available { value, truth },
                    ) => {
                        if truth.origin != LiveGpuTruthOriginV3::Observed {
                            return Err(LiveGpuValidationErrorV3::InvalidTruthEvidence);
                        }
                        validate_anchor(value, session, limits)
                    }
                    (
                        LiveGpuSessionStateV3::Running
                        | LiveGpuSessionStateV3::Terminated
                        | LiveGpuSessionStateV3::Poisoned,
                        LiveGpuAvailabilityV3::Unavailable { .. },
                    ) => Ok(()),
                    _ => Err(LiveGpuValidationErrorV3::InvalidAvailability),
                }
            }
            Self::Hardware { hardware } => validate_hardware_result(hardware, session, limits),
            Self::Scopes {
                anchor,
                page,
                items,
                ..
            } => {
                validate_anchor(anchor, session, limits)?;
                page.validate(items.len(), limits)?;
                if page.snapshot_identity != anchor.snapshot_identity {
                    return Err(LiveGpuValidationErrorV3::IdentityMismatch("snapshot"));
                }
                for item in items {
                    item.validate(anchor, limits)?;
                }
                Ok(())
            }
            Self::Registers {
                anchor,
                scope,
                page,
                items,
            } => {
                validate_scoped_page(anchor, *scope, *page, items.len(), session, limits)?;
                let mut identities = BTreeSet::new();
                for item in items {
                    item.validate(limits)?;
                    if !identities.insert(item.register_identity) {
                        return Err(LiveGpuValidationErrorV3::DuplicateIdentity("register"));
                    }
                }
                Ok(())
            }
            Self::Values {
                anchor,
                scope,
                page,
                items,
            } => {
                validate_scoped_page(anchor, *scope, *page, items.len(), session, limits)?;
                let mut identities = BTreeSet::new();
                for item in items {
                    item.validate(limits)?;
                    if !identities.insert(item.value_identity) {
                        return Err(LiveGpuValidationErrorV3::DuplicateIdentity("value"));
                    }
                }
                Ok(())
            }
            Self::Memory {
                anchor,
                scope,
                memory,
            } => {
                validate_scoped(anchor, *scope, session, limits)?;
                memory.validate(limits)
            }
            Self::ProgramSite {
                anchor,
                scope,
                site,
            } => {
                validate_scoped(anchor, *scope, session, limits)?;
                site.validate(&anchor.binding, limits)
            }
            Self::Terminated if session.state == LiveGpuSessionStateV3::Terminated => Ok(()),
            Self::Terminated => Err(LiveGpuValidationErrorV3::InvalidSessionState),
        }
    }
}

fn validate_hardware_result(
    hardware: &HardwareDebugResultV2,
    session: LiveGpuSessionViewV3,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<(), LiveGpuValidationErrorV3> {
    let operation = match hardware {
        HardwareDebugResultV2::Devices { .. } => HardwareDebugOperationV2::InspectHardwareDevices,
        HardwareDebugResultV2::Queues { .. } => HardwareDebugOperationV2::InspectHardwareQueues,
        HardwareDebugResultV2::Events { .. } => {
            HardwareDebugOperationV2::QueryHardwareExceptionEvents
        }
        HardwareDebugResultV2::QueueControl { .. } => HardwareDebugOperationV2::SuspendQueues,
        HardwareDebugResultV2::Terminated => HardwareDebugOperationV2::Terminate,
        HardwareDebugResultV2::Capabilities { .. } | HardwareDebugResultV2::State => {
            return Err(LiveGpuValidationErrorV3::HardwareV2Rejected);
        }
    };
    let state = match session.state {
        LiveGpuSessionStateV3::Running | LiveGpuSessionStateV3::Stopped => {
            HardwareSessionStateV2::Running
        }
        LiveGpuSessionStateV3::Terminated => HardwareSessionStateV2::Terminated,
        LiveGpuSessionStateV3::Poisoned => HardwareSessionStateV2::Poisoned,
    };
    let response = HardwareDebugResponseV2::Ok {
        schema: HardwareResponseSchemaV2::V2,
        request_id: 1,
        operation,
        session: HardwareSessionViewV2 {
            state,
            commands_processed: session.commands_processed,
            control_revision: session.revision,
            observation_sequence: session.observation_sequence,
            identity_generation: session.identity_generation,
            runtime_enabled: session.runtime_enabled,
            hardware_observed: true,
            simulated: false,
            performance_prediction: false,
        },
        result: hardware.clone(),
    };
    response
        .validate(HardwareProtocolLimitsV2 {
            max_page_items: limits.max_page_items,
            ..HardwareProtocolLimitsV2::default()
        })
        .map_err(|_| LiveGpuValidationErrorV3::HardwareV2Rejected)
}

fn validate_anchor(
    anchor: &LiveGpuStoppedAnchorV3,
    session: LiveGpuSessionViewV3,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<(), LiveGpuValidationErrorV3> {
    if session.state != LiveGpuSessionStateV3::Stopped {
        return Err(LiveGpuValidationErrorV3::SessionNotStopped);
    }
    anchor.validate(limits)?;
    if anchor.binding.binding_identity != session.binding_identity {
        return Err(LiveGpuValidationErrorV3::IdentityMismatch("binding"));
    }
    Ok(())
}

fn validate_scoped(
    anchor: &LiveGpuStoppedAnchorV3,
    scope: LiveGpuScopeSelectorV3,
    session: LiveGpuSessionViewV3,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<(), LiveGpuValidationErrorV3> {
    validate_anchor(anchor, session, limits)?;
    scope.validate()?;
    if scope.dispatch() != anchor.dispatch {
        return Err(LiveGpuValidationErrorV3::IdentityMismatch("dispatch"));
    }
    Ok(())
}

fn validate_scoped_page(
    anchor: &LiveGpuStoppedAnchorV3,
    scope: LiveGpuScopeSelectorV3,
    page: LiveGpuPageViewV3,
    item_count: usize,
    session: LiveGpuSessionViewV3,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<(), LiveGpuValidationErrorV3> {
    validate_scoped(anchor, scope, session, limits)?;
    page.validate(item_count, limits)?;
    if page.snapshot_identity != anchor.snapshot_identity {
        return Err(LiveGpuValidationErrorV3::IdentityMismatch("snapshot"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuErrorStageV3 {
    Framing,
    Validation,
    Binding,
    Observation,
    Query,
    Output,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGpuErrorCodeV3 {
    InvalidRequest,
    StaleRevision,
    StaleBinding,
    StaleStop,
    StaleSnapshot,
    UnknownLogicalIdentity,
    ResourceLimit,
    BackendFailure,
    SessionTerminated,
    SessionPoisoned,
    ResponseTooLarge,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveGpuErrorV3 {
    pub stage: LiveGpuErrorStageV3,
    pub code: LiveGpuErrorCodeV3,
    pub effect: HardwareEffectV2,
    pub terminal: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiveGpuDebugResponseV3 {
    Ok {
        schema: LiveGpuResponseSchemaV3,
        request_id: u64,
        operation: LiveGpuOperationV3,
        session: LiveGpuSessionViewV3,
        result: Box<LiveGpuDebugResultV3>,
    },
    Unavailable {
        schema: LiveGpuResponseSchemaV3,
        request_id: u64,
        operation: LiveGpuOperationV3,
        session: LiveGpuSessionViewV3,
        reason: LiveGpuUnavailableReasonV3,
    },
    Error {
        schema: LiveGpuResponseSchemaV3,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        operation: Option<LiveGpuOperationV3>,
        session: LiveGpuSessionViewV3,
        error: LiveGpuErrorV3,
    },
}

impl LiveGpuDebugResponseV3 {
    pub fn validate(
        &self,
        limits: LiveGpuProtocolLimitsV3,
    ) -> Result<(), LiveGpuValidationErrorV3> {
        limits.validate()?;
        match self {
            Self::Ok {
                request_id,
                operation,
                session,
                result,
                ..
            } => {
                if *request_id == 0 {
                    return Err(LiveGpuValidationErrorV3::ZeroRequestId);
                }
                session.validate()?;
                if !result.matches_operation(*operation) {
                    return Err(LiveGpuValidationErrorV3::OperationResultMismatch);
                }
                result.validate(*session, limits)
            }
            Self::Unavailable {
                request_id,
                session,
                ..
            } => {
                if *request_id == 0 {
                    return Err(LiveGpuValidationErrorV3::ZeroRequestId);
                }
                session.validate()
            }
            Self::Error {
                request_id,
                session,
                error,
                ..
            } => {
                if *request_id == Some(0) {
                    return Err(LiveGpuValidationErrorV3::ZeroRequestId);
                }
                if error.terminal != matches!(session.state, LiveGpuSessionStateV3::Poisoned) {
                    return Err(LiveGpuValidationErrorV3::InvalidSessionState);
                }
                session.validate()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGpuValidationErrorV3 {
    LimitOutOfRange(&'static str),
    ZeroRequestId,
    ZeroCount(&'static str),
    CountOutOfRange(&'static str),
    DuplicateIdentity(&'static str),
    InvalidLogicalIdentity(&'static str),
    InvalidAvailability,
    InvalidTruthEvidence,
    InvalidRange(&'static str),
    RangeOverflow(&'static str),
    InvalidText(&'static str),
    InvalidBits,
    InvalidHexBytes,
    InvalidActiveMask,
    IdentityMismatch(&'static str),
    ValueEncodingMismatch,
    UnavailableCarriesValue,
    SessionNotStopped,
    InvalidSessionState,
    HardwareV2Rejected,
    OperationResultMismatch,
}

impl fmt::Display for LiveGpuValidationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid live-GPU debug protocol value: {self:?}")
    }
}

impl std::error::Error for LiveGpuValidationErrorV3 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGpuCodecErrorV3 {
    Validation(LiveGpuValidationErrorV3),
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

impl fmt::Display for LiveGpuCodecErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "live-GPU debug protocol codec failed: {self:?}")
    }
}

impl std::error::Error for LiveGpuCodecErrorV3 {}

pub fn decode_live_gpu_request_line_v3(
    line: &[u8],
    limits: LiveGpuProtocolLimitsV3,
) -> Result<LiveGpuDebugRequestV3, LiveGpuCodecErrorV3> {
    limits.validate().map_err(LiveGpuCodecErrorV3::Validation)?;
    let payload = validate_line(line, limits.max_request_line_bytes)?;
    let request: LiveGpuDebugRequestV3 =
        serde_json::from_slice(payload).map_err(|_| LiveGpuCodecErrorV3::InvalidJson)?;
    request
        .validate(limits)
        .map_err(LiveGpuCodecErrorV3::Validation)?;
    Ok(request)
}

pub fn decode_live_gpu_response_line_v3(
    line: &[u8],
    limits: LiveGpuProtocolLimitsV3,
) -> Result<LiveGpuDebugResponseV3, LiveGpuCodecErrorV3> {
    limits.validate().map_err(LiveGpuCodecErrorV3::Validation)?;
    let payload = validate_line(line, limits.max_response_line_bytes)?;
    let response: LiveGpuDebugResponseV3 =
        serde_json::from_slice(payload).map_err(|_| LiveGpuCodecErrorV3::InvalidJson)?;
    response
        .validate(limits)
        .map_err(LiveGpuCodecErrorV3::Validation)?;
    Ok(response)
}

pub fn read_live_gpu_request_line_v3<R: BufRead>(
    reader: &mut R,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<Option<LiveGpuDebugRequestV3>, LiveGpuCodecErrorV3> {
    limits.validate().map_err(LiveGpuCodecErrorV3::Validation)?;
    let mut line = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| LiveGpuCodecErrorV3::InputRead)?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(LiveGpuCodecErrorV3::MissingLineTerminator)
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
            return decode_live_gpu_request_line_v3(&line, limits).map(Some);
        }
    }
}

pub fn encode_live_gpu_response_line_v3(
    response: &LiveGpuDebugResponseV3,
    limits: LiveGpuProtocolLimitsV3,
) -> Result<Vec<u8>, LiveGpuCodecErrorV3> {
    response
        .validate(limits)
        .map_err(LiveGpuCodecErrorV3::Validation)?;
    let payload_limit = limits
        .max_response_line_bytes
        .checked_sub(1)
        .ok_or(LiveGpuCodecErrorV3::ResponseTooLarge)?;
    let mut output = Vec::new();
    let mut writer = BoundedWriterV3 {
        output: &mut output,
        max: payload_limit,
        limit_exceeded: false,
        allocation_failed: false,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(if writer.limit_exceeded {
            LiveGpuCodecErrorV3::ResponseTooLarge
        } else if writer.allocation_failed {
            LiveGpuCodecErrorV3::AllocationFailure
        } else {
            LiveGpuCodecErrorV3::JsonEncode
        });
    }
    append_bounded(&mut output, b"\n", limits.max_response_line_bytes)?;
    Ok(output)
}

fn validate_allocation(allocation: AllocationIdentityV1) -> Result<(), LiveGpuValidationErrorV3> {
    if allocation.ordinal == 0 {
        Err(LiveGpuValidationErrorV3::InvalidLogicalIdentity(
            "allocation",
        ))
    } else {
        Ok(())
    }
}

fn validate_text(text: &str, field: &'static str) -> Result<(), LiveGpuValidationErrorV3> {
    if text.is_empty()
        || text.len() > MAX_LIVE_GPU_TEXT_BYTES_V3
        || text.chars().any(char::is_control)
    {
        Err(LiveGpuValidationErrorV3::InvalidText(field))
    } else {
        Ok(())
    }
}

fn validate_unique_text_list(
    items: &[String],
    field: &'static str,
) -> Result<(), LiveGpuValidationErrorV3> {
    if items.is_empty() || items.len() > MAX_LIVE_GPU_SELECTOR_ITEMS_V3 {
        return Err(LiveGpuValidationErrorV3::CountOutOfRange(field));
    }
    let mut unique = BTreeSet::new();
    for item in items {
        validate_text(item, field)?;
        if !unique.insert(item) {
            return Err(LiveGpuValidationErrorV3::DuplicateIdentity(field));
        }
    }
    Ok(())
}

fn validate_bits(bit_width: u16, bits: &str) -> Result<(), LiveGpuValidationErrorV3> {
    if bit_width == 0 || bit_width > MAX_LIVE_GPU_VALUE_BITS_V3 {
        return Err(LiveGpuValidationErrorV3::InvalidBits);
    }
    let digits = usize::from(bit_width).div_ceil(4);
    if bits.len() != digits || !bits.bytes().all(is_lower_hex) {
        return Err(LiveGpuValidationErrorV3::InvalidBits);
    }
    let unused = digits * 4 - usize::from(bit_width);
    if unused != 0 {
        let first = hex_nibble(bits.as_bytes()[0]).ok_or(LiveGpuValidationErrorV3::InvalidBits)?;
        if first >= (1_u8 << (4 - unused)) {
            return Err(LiveGpuValidationErrorV3::InvalidBits);
        }
    }
    Ok(())
}

fn validate_hex_bytes(bytes: &str, max_bytes: u64) -> Result<(), LiveGpuValidationErrorV3> {
    if !bytes.len().is_multiple_of(2)
        || u64::try_from(bytes.len() / 2).unwrap_or(u64::MAX) > max_bytes
        || !bytes.bytes().all(is_lower_hex)
    {
        return Err(LiveGpuValidationErrorV3::InvalidHexBytes);
    }
    Ok(())
}

const fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn validate_line(line: &[u8], max: usize) -> Result<&[u8], LiveGpuCodecErrorV3> {
    if line.is_empty() {
        return Err(LiveGpuCodecErrorV3::EmptyLine);
    }
    if line.len() > max {
        return Err(LiveGpuCodecErrorV3::LineTooLarge);
    }
    let payload = line
        .strip_suffix(b"\n")
        .ok_or(LiveGpuCodecErrorV3::MissingLineTerminator)?;
    if payload.is_empty() {
        return Err(LiveGpuCodecErrorV3::EmptyLine);
    }
    if payload.iter().any(|byte| matches!(*byte, b'\n' | b'\r')) {
        return Err(LiveGpuCodecErrorV3::EmbeddedLineBreak);
    }
    Ok(payload)
}

fn append_bounded(
    output: &mut Vec<u8>,
    bytes: &[u8],
    max: usize,
) -> Result<(), LiveGpuCodecErrorV3> {
    let required = output
        .len()
        .checked_add(bytes.len())
        .ok_or(LiveGpuCodecErrorV3::LineTooLarge)?;
    if required > max {
        return Err(LiveGpuCodecErrorV3::LineTooLarge);
    }
    if required > output.capacity() {
        output
            .try_reserve_exact(required - output.capacity())
            .map_err(|_| LiveGpuCodecErrorV3::AllocationFailure)?;
    }
    output.extend_from_slice(bytes);
    Ok(())
}

struct BoundedWriterV3<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    limit_exceeded: bool,
    allocation_failed: bool,
}

impl Write for BoundedWriterV3<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.limit_exceeded = true;
            return Err(io::Error::other("live-GPU response overflow"));
        };
        if required > self.max {
            self.limit_exceeded = true;
            return Err(io::Error::other("live-GPU response exceeds bound"));
        }
        if required > self.output.capacity()
            && self
                .output
                .try_reserve_exact(required - self.output.capacity())
                .is_err()
        {
            self.allocation_failed = true;
            return Err(io::Error::other("live-GPU response allocation failed"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
