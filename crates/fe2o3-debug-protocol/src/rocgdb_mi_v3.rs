//! Sanitized records exchanged with the debugger-side ROCgdb MI adapter.
//!
//! These records deliberately cannot carry an OS process identifier, native
//! thread identifier, absolute address, pathname, file descriptor, or ROCgdb
//! breakpoint number. The adapter owns those authorities and maps them to
//! per-session logical identities and artifact/allocation-relative locations.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    AllocationIdentityV1, LiveGpuAvailabilityV3, LiveGpuCapabilityAvailabilityV3,
    LiveGpuContentIdentityV3, LiveGpuMemoryReadV3, LiveGpuRegisterValueV3, LiveGpuRelativePcV3,
    LiveGpuSemanticValueV3, LiveGpuSourceSpanV3, LiveGpuTruthOriginV3, LiveGpuTruthV3,
    LiveGpuUnavailableReasonV3, LiveGpuValueEncodingV3, LiveGpuValueKindV3,
    MAX_LIVE_GPU_TEXT_BYTES_V3, MAX_LIVE_GPU_VALUE_BITS_V3, OpaqueIdentityV1,
};

pub const MAX_ROCGDB_MI_CAPABILITIES_V3: usize = 32;
pub const MAX_ROCGDB_MI_THREADS_V3: usize = 256;
pub const MAX_ROCGDB_MI_LANES_V3: usize = 64;
pub const MAX_ROCGDB_MI_REGISTERS_V3: usize = 1_024;
pub const MAX_ROCGDB_MI_VALUES_V3: usize = 512;
pub const MAX_ROCGDB_MI_MEMORY_BYTES_V3: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiCapabilityNameV3 {
    Launch,
    Attach,
    AsyncExecution,
    StructuredThreads,
    StoppedWave,
    LogicalLanes,
    RelativeProgramCounter,
    SourceSite,
    RegisterValues,
    SemanticValues,
    AllocationRelativeMemory,
    Breakpoints,
    Continue,
    Pause,
    Step,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiAuthorizationRequirementV3 {
    NotRequired,
    Required,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiCapabilityV3 {
    pub name: RocgdbMiCapabilityNameV3,
    pub availability: LiveGpuCapabilityAvailabilityV3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<LiveGpuUnavailableReasonV3>,
    pub authorization: RocgdbMiAuthorizationRequirementV3,
}

impl RocgdbMiCapabilityV3 {
    pub fn validate(self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if matches!(
            self.availability,
            LiveGpuCapabilityAvailabilityV3::Available
        ) == self.unavailable_reason.is_some()
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidAvailability);
        }
        let control = matches!(
            self.name,
            RocgdbMiCapabilityNameV3::Launch
                | RocgdbMiCapabilityNameV3::Attach
                | RocgdbMiCapabilityNameV3::Breakpoints
                | RocgdbMiCapabilityNameV3::Continue
                | RocgdbMiCapabilityNameV3::Pause
                | RocgdbMiCapabilityNameV3::Step
        );
        if control
            != matches!(
                self.authorization,
                RocgdbMiAuthorizationRequirementV3::Required
            )
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidAuthorization);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiCapabilitiesV3 {
    pub capabilities: Vec<RocgdbMiCapabilityV3>,
}

impl RocgdbMiCapabilitiesV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if self.capabilities.is_empty() || self.capabilities.len() > MAX_ROCGDB_MI_CAPABILITIES_V3 {
            return Err(RocgdbMiProtocolErrorV3::CountOutOfRange("capabilities"));
        }
        let mut names = BTreeSet::new();
        for capability in self.capabilities.iter().copied() {
            capability.validate()?;
            if !names.insert(capability.name) {
                return Err(RocgdbMiProtocolErrorV3::DuplicateIdentity("capability"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiThreadIdentityV3 {
    pub identity: OpaqueIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiThreadAdmissionV3 {
    /// Digest of the exact structured `-thread-info` result record.
    pub thread_info_record_identity: OpaqueIdentityV1,
    /// Caller-selected tuple ordinal; no GPU classification is inferred from
    /// `target-id`, `details`, names, or stream prose.
    pub thread_ordinal: u16,
    pub thread: RocgdbMiThreadIdentityV3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiWaveIdentityV3 {
    pub identity: OpaqueIdentityV1,
    pub thread: RocgdbMiThreadIdentityV3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiLaneIdentityV3 {
    pub identity: OpaqueIdentityV1,
    pub wave: RocgdbMiWaveIdentityV3,
    pub lane: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiLaneObservationV3 {
    pub lane: RocgdbMiLaneIdentityV3,
    pub active: LiveGpuAvailabilityV3<bool>,
}

impl RocgdbMiLaneObservationV3 {
    fn validate(&self, wave_width: u16) -> Result<(), RocgdbMiProtocolErrorV3> {
        if self.lane.lane >= wave_width {
            return Err(RocgdbMiProtocolErrorV3::CountOutOfRange("lane"));
        }
        validate_availability(&self.active)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiStoppedThreadV3 {
    pub thread: RocgdbMiThreadIdentityV3,
    pub wave: RocgdbMiWaveIdentityV3,
    pub wave_width: u16,
    pub lanes: Vec<RocgdbMiLaneObservationV3>,
    pub relative_pc: LiveGpuAvailabilityV3<LiveGpuRelativePcV3>,
    pub source: LiveGpuAvailabilityV3<LiveGpuSourceSpanV3>,
}

impl RocgdbMiStoppedThreadV3 {
    fn validate(&self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if !matches!(self.wave_width, 32 | 64)
            || self.wave.thread != self.thread
            || self.lanes.len() != usize::from(self.wave_width)
            || self.lanes.len() > MAX_ROCGDB_MI_LANES_V3
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidScope);
        }
        let mut lanes = BTreeSet::new();
        for lane in &self.lanes {
            lane.validate(self.wave_width)?;
            if lane.lane.wave != self.wave || !lanes.insert(lane.lane.lane) {
                return Err(RocgdbMiProtocolErrorV3::InvalidScope);
            }
        }
        validate_availability(&self.relative_pc)?;
        validate_availability(&self.source)?;
        if let LiveGpuAvailabilityV3::Available { value, .. } = self.source
            && value.byte_start >= value.byte_end
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidAvailability);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiStopReasonV3 {
    Breakpoint,
    Step,
    Signal,
    Exited,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiStoppedSnapshotV3 {
    pub snapshot_identity: OpaqueIdentityV1,
    pub stop_identity: OpaqueIdentityV1,
    pub revision: u64,
    pub reason: RocgdbMiStopReasonV3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoint: Option<RocgdbMiBreakpointIdentityV3>,
    pub threads: Vec<RocgdbMiStoppedThreadV3>,
}

impl RocgdbMiStoppedSnapshotV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if self.revision == 0
            || self.threads.is_empty()
            || self.threads.len() > MAX_ROCGDB_MI_THREADS_V3
            || (self.reason == RocgdbMiStopReasonV3::Breakpoint) != self.breakpoint.is_some()
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidSnapshot);
        }
        let mut threads = BTreeSet::new();
        let mut waves = BTreeSet::new();
        for thread in &self.threads {
            thread.validate()?;
            if !threads.insert(thread.thread) || !waves.insert(thread.wave.identity) {
                return Err(RocgdbMiProtocolErrorV3::DuplicateIdentity("thread"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiExecutionEventV3 {
    Running {
        revision: u64,
    },
    Stopped {
        snapshot: RocgdbMiStoppedSnapshotV3,
    },
    Unavailable {
        revision: u64,
        reason: LiveGpuUnavailableReasonV3,
    },
    Exited {
        revision: u64,
    },
}

impl RocgdbMiExecutionEventV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiProtocolErrorV3> {
        match self {
            Self::Running { revision }
            | Self::Unavailable { revision, .. }
            | Self::Exited { revision }
                if *revision == 0 =>
            {
                Err(RocgdbMiProtocolErrorV3::InvalidSnapshot)
            }
            Self::Stopped { snapshot } => snapshot.validate(),
            Self::Running { .. } | Self::Unavailable { .. } | Self::Exited { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiStoppedScopeV3 {
    pub stop_identity: OpaqueIdentityV1,
    pub thread: RocgdbMiThreadIdentityV3,
    pub wave: RocgdbMiWaveIdentityV3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<RocgdbMiLaneIdentityV3>,
}

impl RocgdbMiStoppedScopeV3 {
    pub fn validate(self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if self.wave.thread != self.thread || self.lane.is_some_and(|lane| lane.wave != self.wave) {
            return Err(RocgdbMiProtocolErrorV3::InvalidScope);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiRegisterSnapshotV3 {
    pub scope: RocgdbMiStoppedScopeV3,
    pub registers: Vec<LiveGpuRegisterValueV3>,
}

impl RocgdbMiRegisterSnapshotV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiProtocolErrorV3> {
        self.scope.validate()?;
        if self.registers.len() > MAX_ROCGDB_MI_REGISTERS_V3 {
            return Err(RocgdbMiProtocolErrorV3::CountOutOfRange("registers"));
        }
        let mut identities = BTreeSet::new();
        for register in &self.registers {
            if !identities.insert(register.register_identity) {
                return Err(RocgdbMiProtocolErrorV3::DuplicateIdentity("register"));
            }
            validate_text(&register.name)?;
            if register.lane.is_some_and(|lane| lane >= 64) {
                return Err(RocgdbMiProtocolErrorV3::CountOutOfRange("register lane"));
            }
            validate_value(register.kind, &register.value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiValueSnapshotV3 {
    pub scope: RocgdbMiStoppedScopeV3,
    pub values: Vec<LiveGpuSemanticValueV3>,
}

impl RocgdbMiValueSnapshotV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiProtocolErrorV3> {
        self.scope.validate()?;
        if self.values.len() > MAX_ROCGDB_MI_VALUES_V3 {
            return Err(RocgdbMiProtocolErrorV3::CountOutOfRange("values"));
        }
        let mut identities = BTreeSet::new();
        for value in &self.values {
            if !identities.insert(value.value_identity) {
                return Err(RocgdbMiProtocolErrorV3::DuplicateIdentity("value"));
            }
            validate_text(&value.name)?;
            validate_value(value.kind, &value.value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiMemoryReadRequestV3 {
    pub request_id: u64,
    pub expected_revision: u64,
    pub scope: RocgdbMiStoppedScopeV3,
    pub allocation: AllocationIdentityV1,
    pub byte_offset: u64,
    pub byte_len: u64,
}

impl RocgdbMiMemoryReadRequestV3 {
    pub fn validate(self) -> Result<(), RocgdbMiProtocolErrorV3> {
        self.scope.validate()?;
        if self.request_id == 0
            || self.byte_len == 0
            || self.byte_len > MAX_ROCGDB_MI_MEMORY_BYTES_V3
            || self.byte_offset.checked_add(self.byte_len).is_none()
            || self.allocation.ordinal == 0
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidMemoryRange);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiMemoryReadResultV3 {
    pub request_id: u64,
    pub revision: u64,
    pub memory: LiveGpuMemoryReadV3,
}

impl RocgdbMiMemoryReadResultV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if self.request_id == 0
            || self.revision == 0
            || self.memory.allocation.ordinal == 0
            || self.memory.requested_bytes == 0
            || self.memory.requested_bytes > MAX_ROCGDB_MI_MEMORY_BYTES_V3
            || self.memory.returned_bytes > self.memory.requested_bytes
            || self
                .memory
                .byte_offset
                .checked_add(self.memory.returned_bytes)
                .is_none()
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidMemoryRange);
        }
        validate_availability(&self.memory.value)?;
        match &self.memory.value {
            LiveGpuAvailabilityV3::Available { value, .. } => {
                if value.bytes.len() % 2 != 0
                    || value.bytes.len() / 2
                        != usize::try_from(self.memory.returned_bytes).unwrap_or(usize::MAX)
                    || !value
                        .bytes
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                {
                    return Err(RocgdbMiProtocolErrorV3::InvalidMemoryRange);
                }
            }
            LiveGpuAvailabilityV3::Redacted { .. } | LiveGpuAvailabilityV3::Unavailable { .. }
                if self.memory.returned_bytes != 0 =>
            {
                return Err(RocgdbMiProtocolErrorV3::InvalidMemoryRange);
            }
            LiveGpuAvailabilityV3::Redacted { .. } | LiveGpuAvailabilityV3::Unavailable { .. } => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiBreakpointIdentityV3 {
    pub identity: OpaqueIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "site", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiBreakpointSiteV3 {
    CodeObjectRelative {
        code_object: LiveGpuContentIdentityV3,
        kernel_entry_byte_offset: u64,
    },
    Source {
        source: LiveGpuSourceSpanV3,
    },
}

impl RocgdbMiBreakpointSiteV3 {
    pub fn validate(self) -> Result<(), RocgdbMiProtocolErrorV3> {
        match self {
            Self::CodeObjectRelative { code_object, .. } if code_object.canonical_bytes == 0 => {
                Err(RocgdbMiProtocolErrorV3::InvalidSnapshot)
            }
            Self::Source { source } if source.byte_start >= source.byte_end => {
                Err(RocgdbMiProtocolErrorV3::InvalidSnapshot)
            }
            Self::CodeObjectRelative { .. } | Self::Source { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiControlAuthorizationV3 {
    pub authorization_identity: OpaqueIdentityV1,
    pub expected_revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiStepKindV3 {
    Instruction,
    Into,
    Over,
    Out,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiControlRequestV3 {
    Launch {
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
    },
    Attach {
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
    },
    InsertBreakpoint {
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
        site: RocgdbMiBreakpointSiteV3,
    },
    RemoveBreakpoint {
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
        breakpoint: RocgdbMiBreakpointIdentityV3,
    },
    Continue {
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
        focus: RocgdbMiThreadIdentityV3,
    },
    Pause {
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
    },
    Step {
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
        focus: RocgdbMiThreadIdentityV3,
        kind: RocgdbMiStepKindV3,
    },
}

impl RocgdbMiControlRequestV3 {
    pub const fn request_id(self) -> u64 {
        match self {
            Self::Launch { request_id, .. }
            | Self::Attach { request_id, .. }
            | Self::InsertBreakpoint { request_id, .. }
            | Self::RemoveBreakpoint { request_id, .. }
            | Self::Continue { request_id, .. }
            | Self::Pause { request_id, .. }
            | Self::Step { request_id, .. } => request_id,
        }
    }

    pub const fn authorization(self) -> RocgdbMiControlAuthorizationV3 {
        match self {
            Self::Launch { authorization, .. }
            | Self::Attach { authorization, .. }
            | Self::InsertBreakpoint { authorization, .. }
            | Self::RemoveBreakpoint { authorization, .. }
            | Self::Continue { authorization, .. }
            | Self::Pause { authorization, .. }
            | Self::Step { authorization, .. } => authorization,
        }
    }

    pub fn validate(self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if self.request_id() == 0 {
            return Err(RocgdbMiProtocolErrorV3::ZeroRequestId);
        }
        if let Self::InsertBreakpoint { site, .. } = self {
            site.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiControlOperationV3 {
    Launch,
    Attach,
    InsertBreakpoint,
    RemoveBreakpoint,
    Continue,
    Pause,
    Step,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiControlEffectV3 {
    None,
    Committed,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiControlUnavailableReasonV3 {
    AuthorizationRequired,
    AuthorizationMismatch,
    StaleRevision,
    SessionNotStopped,
    UnknownBreakpoint,
    BackendRejected,
    BackendDisconnected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiControlOutcomeV3 {
    Applied {
        effect: RocgdbMiControlEffectV3,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breakpoint: Option<RocgdbMiBreakpointIdentityV3>,
    },
    Unavailable {
        reason: RocgdbMiControlUnavailableReasonV3,
        effect: RocgdbMiControlEffectV3,
    },
    Failed {
        reason: RocgdbMiControlUnavailableReasonV3,
        effect: RocgdbMiControlEffectV3,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiControlResultV3 {
    pub request_id: u64,
    pub operation: RocgdbMiControlOperationV3,
    pub revision: u64,
    pub outcome: RocgdbMiControlOutcomeV3,
    pub audit: RocgdbMiControlAuditV3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiControlAuditV3 {
    pub audit_identity: OpaqueIdentityV1,
    pub authorization_identity: OpaqueIdentityV1,
    pub before_revision: u64,
    pub after_revision: u64,
    pub effect: RocgdbMiControlEffectV3,
}

impl RocgdbMiControlResultV3 {
    pub fn validate(self) -> Result<(), RocgdbMiProtocolErrorV3> {
        if self.request_id == 0 {
            return Err(RocgdbMiProtocolErrorV3::ZeroRequestId);
        }
        let effect = match self.outcome {
            RocgdbMiControlOutcomeV3::Applied { effect, .. }
            | RocgdbMiControlOutcomeV3::Unavailable { effect, .. }
            | RocgdbMiControlOutcomeV3::Failed { effect, .. } => effect,
        };
        if self.audit.effect != effect
            || self.audit.after_revision != self.revision
            || self.audit.before_revision > self.audit.after_revision
            || (effect == RocgdbMiControlEffectV3::Committed
                && self.audit.after_revision != self.audit.before_revision.saturating_add(1))
            || (effect == RocgdbMiControlEffectV3::None
                && self.audit.after_revision != self.audit.before_revision)
        {
            return Err(RocgdbMiProtocolErrorV3::InvalidEffect);
        }
        match self.outcome {
            RocgdbMiControlOutcomeV3::Applied { effect, breakpoint } => {
                if effect != RocgdbMiControlEffectV3::Committed
                    || (self.operation == RocgdbMiControlOperationV3::InsertBreakpoint)
                        != breakpoint.is_some()
                {
                    return Err(RocgdbMiProtocolErrorV3::InvalidEffect);
                }
            }
            RocgdbMiControlOutcomeV3::Unavailable { effect, .. }
                if effect != RocgdbMiControlEffectV3::None =>
            {
                return Err(RocgdbMiProtocolErrorV3::InvalidEffect);
            }
            RocgdbMiControlOutcomeV3::Failed {
                effect: RocgdbMiControlEffectV3::Committed,
                ..
            } => {
                return Err(RocgdbMiProtocolErrorV3::InvalidEffect);
            }
            RocgdbMiControlOutcomeV3::Unavailable { .. }
            | RocgdbMiControlOutcomeV3::Failed { .. } => {}
        }
        Ok(())
    }
}

fn validate_availability<T>(
    availability: &LiveGpuAvailabilityV3<T>,
) -> Result<(), RocgdbMiProtocolErrorV3> {
    let truth = match availability {
        LiveGpuAvailabilityV3::Available { truth, .. }
        | LiveGpuAvailabilityV3::Redacted { truth, .. } => {
            if truth.origin == LiveGpuTruthOriginV3::Unavailable {
                return Err(RocgdbMiProtocolErrorV3::InvalidAvailability);
            }
            truth
        }
        LiveGpuAvailabilityV3::Unavailable { truth, .. } => {
            if truth.origin != LiveGpuTruthOriginV3::Unavailable {
                return Err(RocgdbMiProtocolErrorV3::InvalidAvailability);
            }
            truth
        }
    };
    validate_truth(truth)
}

fn validate_truth(truth: &LiveGpuTruthV3) -> Result<(), RocgdbMiProtocolErrorV3> {
    match truth.origin {
        LiveGpuTruthOriginV3::Unavailable if truth.evidence.is_empty() => Ok(()),
        LiveGpuTruthOriginV3::Observed
            if truth.evidence.len() == 1
                && truth.evidence[0].kind == crate::LiveGpuEvidenceKindV3::RuntimeObservation =>
        {
            Ok(())
        }
        _ => Err(RocgdbMiProtocolErrorV3::InvalidTruth),
    }
}

fn validate_text(text: &str) -> Result<(), RocgdbMiProtocolErrorV3> {
    if text.is_empty()
        || text.len() > MAX_LIVE_GPU_TEXT_BYTES_V3
        || text.chars().any(char::is_control)
    {
        Err(RocgdbMiProtocolErrorV3::InvalidAvailability)
    } else {
        Ok(())
    }
}

fn validate_value(
    kind: LiveGpuValueKindV3,
    availability: &LiveGpuAvailabilityV3<LiveGpuValueEncodingV3>,
) -> Result<(), RocgdbMiProtocolErrorV3> {
    validate_availability(availability)?;
    let LiveGpuAvailabilityV3::Available { value, .. } = availability else {
        return Ok(());
    };
    let valid = match (kind, value) {
        (
            LiveGpuValueKindV3::Boolean
            | LiveGpuValueKindV3::SignedInteger
            | LiveGpuValueKindV3::UnsignedInteger
            | LiveGpuValueKindV3::FloatingPoint,
            LiveGpuValueEncodingV3::Bits { bit_width, bits },
        ) => {
            *bit_width > 0
                && *bit_width <= MAX_LIVE_GPU_VALUE_BITS_V3
                && bits.len() == usize::from(*bit_width).div_ceil(4)
                && bits
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        }
        (
            LiveGpuValueKindV3::AllocationRelativePointer,
            LiveGpuValueEncodingV3::AllocationRelative { allocation, .. },
        ) => allocation.ordinal != 0,
        (LiveGpuValueKindV3::Bytes, LiveGpuValueEncodingV3::Bytes { bytes }) => {
            bytes.len() % 2 == 0
                && u64::try_from(bytes.len() / 2).unwrap_or(u64::MAX)
                    <= MAX_ROCGDB_MI_MEMORY_BYTES_V3
                && bytes
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(RocgdbMiProtocolErrorV3::InvalidAvailability)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbMiProtocolErrorV3 {
    ZeroRequestId,
    CountOutOfRange(&'static str),
    DuplicateIdentity(&'static str),
    InvalidAvailability,
    InvalidAuthorization,
    InvalidTruth,
    InvalidScope,
    InvalidSnapshot,
    InvalidMemoryRange,
    InvalidEffect,
}

impl std::fmt::Display for RocgdbMiProtocolErrorV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid ROCgdb MI adapter record: {self:?}")
    }
}

impl std::error::Error for RocgdbMiProtocolErrorV3 {}
