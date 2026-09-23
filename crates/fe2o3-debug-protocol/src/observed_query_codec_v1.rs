//! Bounded NDJSON framing for additive observed-query schema families.
use crate::{
    DebugRequestAnyV2, ProtocolCodecErrorV1, ProtocolLimitsV1, RESOURCE_REQUEST_SCHEMA_V2,
    RUNTIME_OBSERVATION_REQUEST_SCHEMA_V1, ResourceRequestV2, ResourceResponseV2,
    RuntimeObservationRequestV1, RuntimeObservationResponseV1,
};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

/// Wrap the old enum instead of adding variants to a published exhaustive enum.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebugRequestAnyV3 {
    Legacy(DebugRequestAnyV2),
    RuntimeObservationV1(RuntimeObservationRequestV1),
    ResourceV2(ResourceRequestV2),
}

fn payload(line: &[u8], max: usize) -> Result<&[u8], ProtocolCodecErrorV1> {
    if line.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if line.len() > max {
        return Err(ProtocolCodecErrorV1::LineTooLarge);
    }
    let bytes = line
        .strip_suffix(b"\n")
        .ok_or(ProtocolCodecErrorV1::MissingLineTerminator)?;
    if bytes.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if bytes.iter().any(|b| matches!(b, b'\n' | b'\r')) {
        return Err(ProtocolCodecErrorV1::EmbeddedLineBreak);
    }
    Ok(bytes)
}
macro_rules! decoder {
    ($name:ident, $ty:ty, $limit:ident) => {
        pub fn $name(line: &[u8], limits: ProtocolLimitsV1) -> Result<$ty, ProtocolCodecErrorV1> {
            limits
                .validate()
                .map_err(ProtocolCodecErrorV1::Validation)?;
            let body = payload(line, limits.$limit)?;
            let value: $ty =
                serde_json::from_slice(body).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
            value
                .validate(limits)
                .map_err(ProtocolCodecErrorV1::Validation)?;
            Ok(value)
        }
    };
}
decoder!(
    decode_runtime_observation_request_line_v1,
    RuntimeObservationRequestV1,
    max_request_line_bytes
);
decoder!(
    decode_runtime_observation_response_line_v1,
    RuntimeObservationResponseV1,
    max_response_line_bytes
);
decoder!(
    decode_resource_request_line_v2,
    ResourceRequestV2,
    max_request_line_bytes
);
decoder!(
    decode_resource_response_line_v2,
    ResourceResponseV2,
    max_response_line_bytes
);

pub fn read_request_line_any_v3<R: BufRead>(
    reader: &mut R,
    limits: ProtocolLimitsV1,
) -> Result<Option<DebugRequestAnyV3>, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let mut line = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| ProtocolCodecErrorV1::InputRead)?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(ProtocolCodecErrorV1::MissingLineTerminator)
            };
        }
        let newline = available.iter().position(|b| *b == b'\n');
        let consumed = newline.map_or(available.len(), |n| n + 1);
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
    #[derive(Deserialize)]
    struct Probe {
        schema: String,
    }
    let probe: Probe = serde_json::from_slice(payload(&line, limits.max_request_line_bytes)?)
        .map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    match probe.schema.as_str() {
        RUNTIME_OBSERVATION_REQUEST_SCHEMA_V1 => {
            decode_runtime_observation_request_line_v1(&line, limits)
                .map(DebugRequestAnyV3::RuntimeObservationV1)
                .map(Some)
        }
        RESOURCE_REQUEST_SCHEMA_V2 => decode_resource_request_line_v2(&line, limits)
            .map(DebugRequestAnyV3::ResourceV2)
            .map(Some),
        _ => crate::read_request_line_any_v2(&mut io::Cursor::new(&line), limits)
            .map(|value| value.map(DebugRequestAnyV3::Legacy)),
    }
}

fn encode<T: Serialize>(
    value: &T,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let mut writer = ObservedBoundedWriter {
        bytes: Vec::new(),
        max: limits
            .max_response_line_bytes
            .checked_sub(1)
            .ok_or(ProtocolCodecErrorV1::ResponseTooLarge)?,
        error: None,
    };
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(writer.error.unwrap_or(ProtocolCodecErrorV1::JsonEncode));
    }
    writer
        .bytes
        .try_reserve_exact(1)
        .map_err(|_| ProtocolCodecErrorV1::AllocationFailure)?;
    writer.bytes.push(b'\n');
    Ok(writer.bytes)
}
pub fn encode_runtime_observation_response_line_v1(
    response: &RuntimeObservationResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    encode(response, limits)
}
pub fn encode_resource_response_line_v2(
    response: &ResourceResponseV2,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    encode(response, limits)
}
struct ObservedBoundedWriter {
    bytes: Vec<u8>,
    max: usize,
    error: Option<ProtocolCodecErrorV1>,
}
impl Write for ObservedBoundedWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let Some(required) = self
            .bytes
            .len()
            .checked_add(input.len())
            .filter(|n| *n <= self.max)
        else {
            self.error = Some(ProtocolCodecErrorV1::ResponseTooLarge);
            return Err(io::Error::other("observed response size limit"));
        };
        if required > self.bytes.capacity()
            && self
                .bytes
                .try_reserve_exact(required - self.bytes.len())
                .is_err()
        {
            self.error = Some(ProtocolCodecErrorV1::AllocationFailure);
            return Err(io::Error::other("observed response allocation failed"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
