//! Separately versioned, read-only source-variable inspection protocol.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::{
    DebugErrorV1, DebugRequestV1, DebugSnapshotAnchorV1, ExecutionScopeSelectorV1,
    OpaqueIdentityV1, PageCursorV1, PageRequestV1, ProtocolCodecErrorV1, ProtocolLimitsV1,
    ProtocolValidationErrorV1, SessionViewV1, ValueAvailabilityV1, ValueUnavailableReasonV1,
};

pub const SOURCE_VARIABLE_REQUEST_SCHEMA_V2: &str = "fe2o3-debug-source-variable-request-v2";
pub const SOURCE_VARIABLE_RESPONSE_SCHEMA_V2: &str = "fe2o3-debug-source-variable-response-v2";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SourceVariableRequestSchemaV2 {
    #[serde(rename = "fe2o3-debug-source-variable-request-v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SourceVariableResponseSchemaV2 {
    #[serde(rename = "fe2o3-debug-source-variable-response-v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceVariableOperationV2 {
    InspectSourceVariables,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "selector", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceVariableSelectorV2 {
    All,
    Identity { variable_identity: OpaqueIdentityV1 },
    Name { name: String },
}

impl SourceVariableSelectorV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if let Self::Name { name } = self
            && (name.is_empty()
                || name.len() > crate::MAX_TEXT_BYTES_V1
                || name.chars().any(char::is_control))
        {
            return Err(ProtocolValidationErrorV1::InvalidText(
                "source variable name",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceVariableRequestV2 {
    InspectSourceVariables {
        schema: SourceVariableRequestSchemaV2,
        request_id: u64,
        expected_revision: u64,
        scope: ExecutionScopeSelectorV1,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        selector: SourceVariableSelectorV2,
        page: PageRequestV1,
    },
}

impl SourceVariableRequestV2 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::InspectSourceVariables { request_id, .. } => *request_id,
        }
    }

    pub const fn expected_revision(&self) -> u64 {
        match self {
            Self::InspectSourceVariables {
                expected_revision, ..
            } => *expected_revision,
        }
    }

    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        match self {
            Self::InspectSourceVariables {
                request_id,
                scope,
                frame,
                selector,
                page,
                ..
            } => {
                if *request_id == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                if frame == &Some(0) {
                    return Err(ProtocolValidationErrorV1::ZeroIdentity);
                }
                scope.validate()?;
                selector.validate()?;
                page.validate()
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceVariableValueV2 {
    pub variable_identity: OpaqueIdentityV1,
    pub name: String,
    pub function_ordinal: u64,
    pub scope_identity: OpaqueIdentityV1,
    pub scope_depth: u32,
    pub generation: u64,
    pub availability: SourceVariableValueAvailabilityV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceVariableValueAvailabilityV2 {
    Value { value: ValueAvailabilityV1 },
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceVariableQueryUnavailableReasonV2 {
    SourceMapV2Required,
    VariablesNotCaptured,
    OutsideCaptureScope,
    CheckpointNotCaptured,
    FrameUnavailable,
    NameNotInScope,
}

impl SourceVariableValueV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.name.is_empty()
            || self.name.len() > crate::MAX_TEXT_BYTES_V1
            || self.name.chars().any(char::is_control)
            || self.function_ordinal > u64::from(u32::MAX)
        {
            return Err(ProtocolValidationErrorV1::InvalidText(
                "source variable result",
            ));
        }
        let generation_required = match &self.availability {
            SourceVariableValueAvailabilityV2::Value { value } => {
                value.validate()?;
                matches!(
                    value,
                    ValueAvailabilityV1::Captured { .. } | ValueAvailabilityV1::Redacted { .. }
                ) || matches!(
                    value,
                    ValueAvailabilityV1::Unavailable {
                        reason: ValueUnavailableReasonV1::Uninitialized
                            | ValueUnavailableReasonV1::NotLive
                            | ValueUnavailableReasonV1::Truncated,
                    }
                )
            }
            // Name identity can be ambiguous before either candidate has a
            // live storage generation. Explicit map-level ambiguous bindings
            // remain nonzero by Source Map V2 validation.
            SourceVariableValueAvailabilityV2::Ambiguous => false,
        };
        if generation_required && self.generation == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceVariableResponseV2 {
    Ok {
        schema: SourceVariableResponseSchemaV2,
        request_id: u64,
        operation: SourceVariableOperationV2,
        session: SessionViewV1,
        snapshot: Box<DebugSnapshotAnchorV1>,
        values: Vec<SourceVariableValueV2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<PageCursorV1>,
    },
    Unavailable {
        schema: SourceVariableResponseSchemaV2,
        request_id: u64,
        operation: SourceVariableOperationV2,
        session: SessionViewV1,
        reason: SourceVariableQueryUnavailableReasonV2,
    },
    Error {
        schema: SourceVariableResponseSchemaV2,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
        operation: SourceVariableOperationV2,
        session: SessionViewV1,
        error: DebugErrorV1,
    },
}

impl SourceVariableResponseV2 {
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        match self {
            Self::Ok {
                request_id,
                session,
                snapshot,
                values,
                next_cursor,
                ..
            } => {
                if *request_id == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                session.validate()?;
                snapshot.validate()?;
                if snapshot.cursor != session.cursor {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "source variable snapshot cursor",
                    ));
                }
                if values.len() > limits.max_response_items {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange(
                        "source variable values",
                    ));
                }
                for value in values {
                    value.validate()?;
                }
                if next_cursor.is_some_and(|cursor| cursor.position == 0) {
                    return Err(ProtocolValidationErrorV1::ZeroIdentity);
                }
                Ok(())
            }
            Self::Unavailable {
                request_id,
                session,
                ..
            } => {
                if *request_id == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                session.validate()
            }
            Self::Error {
                request_id,
                session,
                error,
                ..
            } => {
                if request_id == &Some(0) {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                session.validate()?;
                error.validate()
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebugRequestAnyV2 {
    V1(DebugRequestV1),
    SourceVariablesV2(SourceVariableRequestV2),
}

pub fn decode_source_variable_request_line_v2(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<SourceVariableRequestV2, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line_v2(line, limits.max_request_line_bytes)?;
    let request: SourceVariableRequestV2 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    request
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(request)
}

pub fn decode_source_variable_response_line_v2(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<SourceVariableResponseV2, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line_v2(line, limits.max_response_line_bytes)?;
    let response: SourceVariableResponseV2 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(response)
}

pub fn read_request_line_any_v2<R: BufRead>(
    reader: &mut R,
    limits: ProtocolLimitsV1,
) -> Result<Option<DebugRequestAnyV2>, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let mut line = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| ProtocolCodecErrorV1::InputRead)?;
        if available.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            return Err(ProtocolCodecErrorV1::MissingLineTerminator);
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |position| position + 1);
        let required = line
            .len()
            .checked_add(consumed)
            .ok_or(ProtocolCodecErrorV1::LineTooLarge)?;
        if required > limits.max_request_line_bytes {
            return Err(ProtocolCodecErrorV1::LineTooLarge);
        }
        line.try_reserve_exact(consumed)
            .map_err(|_| ProtocolCodecErrorV1::AllocationFailure)?;
        line.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }
    let payload = validate_line_v2(&line, limits.max_request_line_bytes)?;
    #[derive(Deserialize)]
    struct SchemaProbe {
        schema: String,
    }
    let probe: SchemaProbe =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    match probe.schema.as_str() {
        crate::REQUEST_SCHEMA_V1 => crate::decode_request_line_v1(&line, limits)
            .map(DebugRequestAnyV2::V1)
            .map(Some),
        SOURCE_VARIABLE_REQUEST_SCHEMA_V2 => decode_source_variable_request_line_v2(&line, limits)
            .map(DebugRequestAnyV2::SourceVariablesV2)
            .map(Some),
        _ => Err(ProtocolCodecErrorV1::InvalidJson),
    }
}

pub fn encode_source_variable_response_line_v2(
    response: &SourceVariableResponseV2,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload_limit = limits
        .max_response_line_bytes
        .checked_sub(1)
        .ok_or(ProtocolCodecErrorV1::ResponseTooLarge)?;
    let mut output = Vec::new();
    let mut writer = BoundedWriterV2 {
        output: &mut output,
        max: payload_limit,
        limit_exceeded: false,
        allocation_failed: false,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(if writer.limit_exceeded {
            ProtocolCodecErrorV1::ResponseTooLarge
        } else if writer.allocation_failed {
            ProtocolCodecErrorV1::AllocationFailure
        } else {
            ProtocolCodecErrorV1::JsonEncode
        });
    }
    output
        .try_reserve_exact(1)
        .map_err(|_| ProtocolCodecErrorV1::AllocationFailure)?;
    output.push(b'\n');
    Ok(output)
}

fn validate_line_v2(line: &[u8], max: usize) -> Result<&[u8], ProtocolCodecErrorV1> {
    if line.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if line.len() > max {
        return Err(ProtocolCodecErrorV1::LineTooLarge);
    }
    let payload = line
        .strip_suffix(b"\n")
        .ok_or(ProtocolCodecErrorV1::MissingLineTerminator)?;
    if payload.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if payload.iter().any(|byte| matches!(*byte, b'\n' | b'\r')) {
        return Err(ProtocolCodecErrorV1::EmbeddedLineBreak);
    }
    Ok(payload)
}

struct BoundedWriterV2<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    limit_exceeded: bool,
    allocation_failed: bool,
}

impl Write for BoundedWriterV2<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let required = match self.output.len().checked_add(bytes.len()) {
            Some(required) if required <= self.max => required,
            _ => {
                self.limit_exceeded = true;
                return Err(io::Error::other("bounded response exceeded"));
            }
        };
        if required > self.output.capacity()
            && self
                .output
                .try_reserve_exact(required - self.output.capacity())
                .is_err()
        {
            self.allocation_failed = true;
            return Err(io::Error::other("bounded response allocation failed"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
