//! Agent-facing JSONL coordinator protocol for the ROCgdb MI substrate.
//!
//! Native addresses and source paths exist only in explicit admission requests.
//! Responses contain logical identities and artifact/allocation-relative facts.

use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::*;

pub const ROCGDB_MI_CLI_REQUEST_SCHEMA_V3: &str = "fe2o3-rocgdb-mi-request-v3";
pub const ROCGDB_MI_CLI_RESPONSE_SCHEMA_V3: &str = "fe2o3-rocgdb-mi-response-v3";
pub const MAX_ROCGDB_MI_CLI_REQUEST_BYTES_V3: usize = 64 * 1024;
pub const MAX_ROCGDB_MI_CLI_RESPONSE_BYTES_V3: usize = 2 * 1024 * 1024;
pub const MAX_ROCGDB_MI_CLI_REQUESTS_V3: u64 = 4_096;
pub const MAX_ROCGDB_MI_CLI_SOURCE_PATH_BYTES_V3: usize = 4_096;
pub const MAX_ROCGDB_MI_CLI_EXPRESSION_BYTES_V3: usize = 32 * 1024;
pub const MAX_ROCGDB_MI_CLI_WAIT_MILLISECONDS_V3: u64 = 60_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RocgdbMiCliRequestSchemaV3 {
    #[serde(rename = "fe2o3-rocgdb-mi-request-v3")]
    V3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RocgdbMiCliResponseSchemaV3 {
    #[serde(rename = "fe2o3-rocgdb-mi-response-v3")]
    V3,
}

/// Canonical lowercase hexadecimal authority accepted only on input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RocgdbMiNativeAddressInputV3(String);

impl RocgdbMiNativeAddressInputV3 {
    pub fn parse(&self) -> Option<u64> {
        let digits = self.0.strip_prefix("0x")?;
        if digits.is_empty()
            || (digits.len() > 1 && digits.starts_with('0'))
            || !digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return None;
        }
        u64::from_str_radix(digits, 16).ok()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiCliRequestV3 {
    GetSession {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
    },
    DiscoverCapabilities {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
    },
    NextEvent {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        wait_milliseconds: u64,
    },
    AdmitThreads {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        thread_ordinals: Vec<u16>,
    },
    AdmitCodeObject {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        content: LiveGpuContentIdentityV3,
        load_base: RocgdbMiNativeAddressInputV3,
        byte_len: u64,
        kernel_entry: RocgdbMiNativeAddressInputV3,
    },
    AdmitAllocation {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        allocation: AllocationIdentityV1,
        base: RocgdbMiNativeAddressInputV3,
        byte_len: u64,
        space: LiveGpuMemorySpaceV3,
    },
    AdmitSourceLine {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        source: LiveGpuSourceSpanV3,
        path: String,
        line: u64,
    },
    InspectRegisters {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        scope: RocgdbMiStoppedScopeV3,
    },
    InspectValues {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        scope: RocgdbMiStoppedScopeV3,
    },
    EvaluateExpression {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        scope: RocgdbMiStoppedScopeV3,
        value_identity: OpaqueIdentityV1,
        name: String,
        expression: String,
    },
    ReadMemory {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        request: RocgdbMiMemoryReadRequestV3,
    },
    Control {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        control: RocgdbMiControlRequestV3,
    },
    Terminate {
        schema: RocgdbMiCliRequestSchemaV3,
        request_id: u64,
        authorization: RocgdbMiControlAuthorizationV3,
    },
}

impl RocgdbMiCliRequestV3 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::GetSession { request_id, .. }
            | Self::DiscoverCapabilities { request_id, .. }
            | Self::NextEvent { request_id, .. }
            | Self::AdmitThreads { request_id, .. }
            | Self::AdmitCodeObject { request_id, .. }
            | Self::AdmitAllocation { request_id, .. }
            | Self::AdmitSourceLine { request_id, .. }
            | Self::InspectRegisters { request_id, .. }
            | Self::InspectValues { request_id, .. }
            | Self::EvaluateExpression { request_id, .. }
            | Self::ReadMemory { request_id, .. }
            | Self::Control { request_id, .. }
            | Self::Terminate { request_id, .. } => *request_id,
        }
    }

    pub fn validate(&self) -> Result<(), RocgdbMiCliValidationErrorV3> {
        if self.request_id() == 0 {
            return Err(RocgdbMiCliValidationErrorV3::ZeroRequestId);
        }
        match self {
            Self::NextEvent {
                wait_milliseconds, ..
            } if *wait_milliseconds == 0
                || *wait_milliseconds > MAX_ROCGDB_MI_CLI_WAIT_MILLISECONDS_V3 =>
            {
                Err(RocgdbMiCliValidationErrorV3::InvalidWait)
            }
            Self::AdmitThreads {
                thread_ordinals, ..
            } => {
                if thread_ordinals.is_empty()
                    || thread_ordinals.len() > MAX_ROCGDB_MI_THREADS_V3
                    || thread_ordinals
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != thread_ordinals.len()
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidAdmission)
                } else {
                    Ok(())
                }
            }
            Self::AdmitCodeObject {
                content,
                load_base,
                byte_len,
                kernel_entry,
                ..
            } => {
                let base = load_base
                    .parse()
                    .ok_or(RocgdbMiCliValidationErrorV3::InvalidNativeInput)?;
                let entry = kernel_entry
                    .parse()
                    .ok_or(RocgdbMiCliValidationErrorV3::InvalidNativeInput)?;
                let end = base
                    .checked_add(*byte_len)
                    .ok_or(RocgdbMiCliValidationErrorV3::InvalidAdmission)?;
                if content.canonical_bytes == 0 || *byte_len == 0 || entry < base || entry >= end {
                    Err(RocgdbMiCliValidationErrorV3::InvalidAdmission)
                } else {
                    Ok(())
                }
            }
            Self::AdmitAllocation {
                allocation,
                base,
                byte_len,
                ..
            } => {
                let base = base
                    .parse()
                    .ok_or(RocgdbMiCliValidationErrorV3::InvalidNativeInput)?;
                if allocation.ordinal == 0
                    || *byte_len == 0
                    || base.checked_add(*byte_len).is_none()
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidAdmission)
                } else {
                    Ok(())
                }
            }
            Self::AdmitSourceLine {
                source, path, line, ..
            } => {
                if *line == 0
                    || source.byte_start >= source.byte_end
                    || path.is_empty()
                    || path.len() > MAX_ROCGDB_MI_CLI_SOURCE_PATH_BYTES_V3
                    || path.chars().any(char::is_control)
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidAdmission)
                } else {
                    Ok(())
                }
            }
            Self::InspectRegisters { scope, .. } | Self::InspectValues { scope, .. } => scope
                .validate()
                .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidScope),
            Self::EvaluateExpression {
                scope,
                name,
                expression,
                ..
            } => {
                scope
                    .validate()
                    .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidScope)?;
                if name.is_empty()
                    || name.len() > MAX_LIVE_GPU_TEXT_BYTES_V3
                    || name.chars().any(char::is_control)
                    || expression.is_empty()
                    || expression.len() > MAX_ROCGDB_MI_CLI_EXPRESSION_BYTES_V3
                    || expression
                        .chars()
                        .any(|character| matches!(character, '\0' | '\r' | '\n'))
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidExpression)
                } else {
                    Ok(())
                }
            }
            Self::ReadMemory { request, .. } => request
                .validate()
                .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidMemory),
            Self::Control {
                request_id,
                control,
                ..
            } => {
                if control.request_id() != *request_id
                    || matches!(
                        control,
                        RocgdbMiControlRequestV3::Launch { .. }
                            | RocgdbMiControlRequestV3::Attach { .. }
                    )
                {
                    return Err(RocgdbMiCliValidationErrorV3::InvalidControl);
                }
                control
                    .validate()
                    .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidControl)
            }
            Self::GetSession { .. }
            | Self::DiscoverCapabilities { .. }
            | Self::Terminate { .. } => Ok(()),
            Self::NextEvent { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiCliCapabilitiesV3 {
    pub mi: RocgdbMiCapabilitiesV3,
    pub generic_stopped_scopes: Vec<LiveGpuCapabilityV3>,
}

impl RocgdbMiCliCapabilitiesV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiCliValidationErrorV3> {
        self.mi
            .validate()
            .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult)?;
        let unavailable_gpu_semantics = [
            RocgdbMiCapabilityNameV3::StoppedWave,
            RocgdbMiCapabilityNameV3::LogicalLanes,
            RocgdbMiCapabilityNameV3::RelativeProgramCounter,
            RocgdbMiCapabilityNameV3::SourceSite,
            RocgdbMiCapabilityNameV3::RegisterValues,
            RocgdbMiCapabilityNameV3::SemanticValues,
            RocgdbMiCapabilityNameV3::AllocationRelativeMemory,
        ];
        if !unavailable_gpu_semantics.iter().all(|name| {
            self.mi.capabilities.iter().any(|capability| {
                capability.name == *name
                    && capability.availability == LiveGpuCapabilityAvailabilityV3::Unavailable
                    && capability.unavailable_reason
                        == Some(LiveGpuUnavailableReasonV3::Unsupported)
            })
        }) {
            return Err(RocgdbMiCliValidationErrorV3::InvalidResult);
        }
        let expected = [
            LiveGpuCapabilityNameV3::StoppedDispatch,
            LiveGpuCapabilityNameV3::StoppedWorkgroups,
            LiveGpuCapabilityNameV3::StoppedWaves,
            LiveGpuCapabilityNameV3::StoppedLanes,
        ];
        if self.generic_stopped_scopes.len() != expected.len()
            || !expected.iter().all(|name| {
                self.generic_stopped_scopes.iter().any(|capability| {
                    capability.backend == LiveGpuBackendV3::RocgdbMi
                        && capability.name == *name
                        && capability.availability == LiveGpuCapabilityAvailabilityV3::Unavailable
                        && capability.unavailable_reason
                            == Some(LiveGpuUnavailableReasonV3::Unsupported)
                })
            })
        {
            return Err(RocgdbMiCliValidationErrorV3::InvalidResult);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "binding", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiCliBindingAdmissionV3 {
    CodeObject { content: LiveGpuContentIdentityV3 },
    Allocation { allocation: AllocationIdentityV1 },
    SourceLine { source: LiveGpuSourceSpanV3 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiCliResultV3 {
    Session {
        session_identity: OpaqueIdentityV1,
        bootstrap: RocgdbMiControlResultV3,
    },
    Capabilities {
        capabilities: RocgdbMiCliCapabilitiesV3,
    },
    Event {
        event: RocgdbMiExecutionEventV3,
    },
    ThreadsAdmitted {
        admissions: Vec<RocgdbMiThreadAdmissionV3>,
    },
    BindingAdmitted {
        admission: RocgdbMiCliBindingAdmissionV3,
    },
    Registers {
        snapshot: RocgdbMiRegisterSnapshotV3,
    },
    Values {
        snapshot: RocgdbMiValueSnapshotV3,
    },
    EvaluatedValue {
        scope: RocgdbMiStoppedScopeV3,
        value: LiveGpuSemanticValueV3,
    },
    Memory {
        memory: RocgdbMiMemoryReadResultV3,
    },
    Control {
        control: RocgdbMiControlResultV3,
    },
    Terminated {
        revision: u64,
        effect: RocgdbMiControlEffectV3,
    },
}

impl RocgdbMiCliResultV3 {
    fn reported_revision(&self) -> Option<u64> {
        match self {
            Self::Event { event } => Some(match event {
                RocgdbMiExecutionEventV3::Running { revision }
                | RocgdbMiExecutionEventV3::Unavailable { revision, .. }
                | RocgdbMiExecutionEventV3::Exited { revision } => *revision,
                RocgdbMiExecutionEventV3::Stopped { snapshot } => snapshot.revision,
            }),
            Self::Memory { memory } => Some(memory.revision),
            Self::Control { control } => Some(control.revision),
            Self::Terminated { revision, .. } => Some(*revision),
            Self::Session { .. }
            | Self::Capabilities { .. }
            | Self::ThreadsAdmitted { .. }
            | Self::BindingAdmitted { .. }
            | Self::Registers { .. }
            | Self::Values { .. }
            | Self::EvaluatedValue { .. } => None,
        }
    }

    pub fn validate(&self) -> Result<(), RocgdbMiCliValidationErrorV3> {
        match self {
            Self::Session { bootstrap, .. } => {
                bootstrap
                    .validate()
                    .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult)?;
                if !matches!(
                    bootstrap.operation,
                    RocgdbMiControlOperationV3::Launch | RocgdbMiControlOperationV3::Attach
                ) {
                    return Err(RocgdbMiCliValidationErrorV3::InvalidResult);
                }
                Ok(())
            }
            Self::Capabilities { capabilities } => capabilities.validate(),
            Self::Event { event } => event
                .validate()
                .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult),
            Self::ThreadsAdmitted { admissions } => {
                if admissions.is_empty()
                    || admissions.len() > MAX_ROCGDB_MI_THREADS_V3
                    || admissions
                        .iter()
                        .map(|item| item.thread)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != admissions.len()
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidResult)
                } else {
                    Ok(())
                }
            }
            Self::BindingAdmitted { admission } => match admission {
                RocgdbMiCliBindingAdmissionV3::CodeObject { content }
                    if content.canonical_bytes == 0 =>
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidResult)
                }
                RocgdbMiCliBindingAdmissionV3::Allocation { allocation }
                    if allocation.ordinal == 0 =>
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidResult)
                }
                RocgdbMiCliBindingAdmissionV3::SourceLine { source }
                    if source.byte_start >= source.byte_end =>
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidResult)
                }
                _ => Ok(()),
            },
            Self::Registers { snapshot } => snapshot
                .validate()
                .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult),
            Self::Values { snapshot } => snapshot
                .validate()
                .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult),
            Self::EvaluatedValue { scope, value } => RocgdbMiValueSnapshotV3 {
                scope: *scope,
                values: vec![value.clone()],
            }
            .validate()
            .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult),
            Self::Memory { memory } => memory
                .validate()
                .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult),
            Self::Control { control } => control
                .validate()
                .map_err(|_| RocgdbMiCliValidationErrorV3::InvalidResult),
            Self::Terminated { revision, effect }
                if *revision == 0 || *effect == RocgdbMiControlEffectV3::None =>
            {
                Err(RocgdbMiCliValidationErrorV3::InvalidResult)
            }
            Self::Terminated { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiCliErrorCodeV3 {
    InvalidRequest,
    DuplicateRequestId,
    CommandBudgetExhausted,
    AuthorizationMismatch,
    StaleRevision,
    SessionNotStopped,
    GpuClassificationUnavailable,
    UnknownLogicalIdentity,
    InvalidBinding,
    BackendRejected,
    BackendDisconnected,
    Timeout,
    ResponseTooLarge,
    SessionTerminated,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiCliResponseV3 {
    Ok {
        schema: RocgdbMiCliResponseSchemaV3,
        request_id: u64,
        revision: u64,
        result: Box<RocgdbMiCliResultV3>,
    },
    Error {
        schema: RocgdbMiCliResponseSchemaV3,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
        revision: u64,
        code: RocgdbMiCliErrorCodeV3,
        effect: RocgdbMiControlEffectV3,
        terminal: bool,
    },
}

impl RocgdbMiCliResponseV3 {
    pub fn validate(&self) -> Result<(), RocgdbMiCliValidationErrorV3> {
        match self {
            Self::Ok {
                request_id,
                revision,
                result,
                ..
            } => {
                if *request_id == 0 || *revision == 0 {
                    return Err(RocgdbMiCliValidationErrorV3::InvalidResult);
                }
                if result
                    .reported_revision()
                    .is_some_and(|reported| reported != *revision)
                {
                    return Err(RocgdbMiCliValidationErrorV3::InvalidResult);
                }
                result.validate()
            }
            Self::Error {
                request_id,
                revision,
                code,
                effect,
                terminal,
                ..
            } => {
                if request_id == &Some(0)
                    || (*revision == 0 && *code != RocgdbMiCliErrorCodeV3::InvalidRequest)
                    || (*code == RocgdbMiCliErrorCodeV3::SessionTerminated && !*terminal)
                    || (*effect == RocgdbMiControlEffectV3::Committed)
                {
                    Err(RocgdbMiCliValidationErrorV3::InvalidResult)
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbMiCliValidationErrorV3 {
    ZeroRequestId,
    InvalidWait,
    InvalidAdmission,
    InvalidNativeInput,
    InvalidScope,
    InvalidExpression,
    InvalidMemory,
    InvalidControl,
    InvalidResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbMiCliCodecErrorV3 {
    EmptyLine,
    LineTooLarge,
    MissingTerminator,
    EmbeddedLineBreak,
    InvalidJson,
    InvalidValue(RocgdbMiCliValidationErrorV3),
    InputRead,
    Output,
    ResponseTooLarge,
}

impl fmt::Display for RocgdbMiCliCodecErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ROCgdb MI JSONL codec failed: {self:?}")
    }
}

impl std::error::Error for RocgdbMiCliCodecErrorV3 {}

pub fn decode_rocgdb_mi_cli_request_line_v3(
    line: &[u8],
) -> Result<RocgdbMiCliRequestV3, RocgdbMiCliCodecErrorV3> {
    let payload = validate_cli_line(line, MAX_ROCGDB_MI_CLI_REQUEST_BYTES_V3)?;
    let request = serde_json::from_slice::<RocgdbMiCliRequestV3>(payload)
        .map_err(|_| RocgdbMiCliCodecErrorV3::InvalidJson)?;
    request
        .validate()
        .map_err(RocgdbMiCliCodecErrorV3::InvalidValue)?;
    Ok(request)
}

pub fn decode_rocgdb_mi_cli_response_line_v3(
    line: &[u8],
) -> Result<RocgdbMiCliResponseV3, RocgdbMiCliCodecErrorV3> {
    let payload = validate_cli_line(line, MAX_ROCGDB_MI_CLI_RESPONSE_BYTES_V3)?;
    let response = serde_json::from_slice::<RocgdbMiCliResponseV3>(payload)
        .map_err(|_| RocgdbMiCliCodecErrorV3::InvalidJson)?;
    response
        .validate()
        .map_err(RocgdbMiCliCodecErrorV3::InvalidValue)?;
    Ok(response)
}

pub fn read_rocgdb_mi_cli_request_line_v3<R: BufRead>(
    reader: &mut R,
) -> Result<Option<RocgdbMiCliRequestV3>, RocgdbMiCliCodecErrorV3> {
    let mut line = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| RocgdbMiCliCodecErrorV3::InputRead)?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(RocgdbMiCliCodecErrorV3::MissingTerminator)
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |position| position + 1);
        if line.len().saturating_add(consumed) > MAX_ROCGDB_MI_CLI_REQUEST_BYTES_V3 {
            return Err(RocgdbMiCliCodecErrorV3::LineTooLarge);
        }
        line.try_reserve(consumed)
            .map_err(|_| RocgdbMiCliCodecErrorV3::InputRead)?;
        line.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if newline.is_some() {
            return decode_rocgdb_mi_cli_request_line_v3(&line).map(Some);
        }
    }
}

pub fn encode_rocgdb_mi_cli_response_line_v3(
    response: &RocgdbMiCliResponseV3,
) -> Result<Vec<u8>, RocgdbMiCliCodecErrorV3> {
    response
        .validate()
        .map_err(RocgdbMiCliCodecErrorV3::InvalidValue)?;
    let max_payload = MAX_ROCGDB_MI_CLI_RESPONSE_BYTES_V3
        .checked_sub(1)
        .ok_or(RocgdbMiCliCodecErrorV3::ResponseTooLarge)?;
    let mut output = Vec::new();
    let mut writer = BoundedResponseWriterV3 {
        output: &mut output,
        max: max_payload,
        limit_exceeded: false,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(if writer.limit_exceeded {
            RocgdbMiCliCodecErrorV3::ResponseTooLarge
        } else {
            RocgdbMiCliCodecErrorV3::Output
        });
    }
    output
        .try_reserve(1)
        .map_err(|_| RocgdbMiCliCodecErrorV3::Output)?;
    if output.len() >= MAX_ROCGDB_MI_CLI_RESPONSE_BYTES_V3 {
        return Err(RocgdbMiCliCodecErrorV3::ResponseTooLarge);
    }
    output.push(b'\n');
    Ok(output)
}

struct BoundedResponseWriterV3<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    limit_exceeded: bool,
}

impl Write for BoundedResponseWriterV3<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(end) = self.output.len().checked_add(bytes.len()) else {
            self.limit_exceeded = true;
            return Err(io::Error::other("response bound exceeded"));
        };
        if end > self.max {
            self.limit_exceeded = true;
            return Err(io::Error::other("response bound exceeded"));
        }
        if self.output.try_reserve(bytes.len()).is_err() {
            return Err(io::Error::other("response allocation failed"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn validate_cli_line(line: &[u8], max: usize) -> Result<&[u8], RocgdbMiCliCodecErrorV3> {
    if line.is_empty() {
        return Err(RocgdbMiCliCodecErrorV3::EmptyLine);
    }
    if line.len() > max {
        return Err(RocgdbMiCliCodecErrorV3::LineTooLarge);
    }
    let payload = line
        .strip_suffix(b"\n")
        .ok_or(RocgdbMiCliCodecErrorV3::MissingTerminator)?;
    if payload.is_empty() {
        return Err(RocgdbMiCliCodecErrorV3::EmptyLine);
    }
    if payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(RocgdbMiCliCodecErrorV3::EmbeddedLineBreak);
    }
    Ok(payload)
}
