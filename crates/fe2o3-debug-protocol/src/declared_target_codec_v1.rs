//! Explicit target-family framing. Older schemas keep their original limits and decoders.
use crate::observed_query_codec_v1::{encode, payload};
use crate::*;
use serde::Deserialize;
use std::io::{self, BufRead};

/// A new wrapper preserves the already-published exhaustive V3 enum unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebugRequestAnyV4 {
    Legacy(DebugRequestAnyV3),
    DeclaredTargetV1(DeclaredTargetRequestV1),
}
fn clipped(limits: ProtocolLimitsV1) -> Result<ProtocolLimitsV1, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(ProtocolLimitsV1 {
        max_request_line_bytes: limits
            .max_request_line_bytes
            .min(MAX_DECLARED_TARGET_LINE_BYTES_V1),
        max_response_line_bytes: limits
            .max_response_line_bytes
            .min(MAX_DECLARED_TARGET_LINE_BYTES_V1),
        ..limits
    })
}
pub fn decode_declared_target_request_line_v1(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<DeclaredTargetRequestV1, ProtocolCodecErrorV1> {
    let limits = clipped(limits)?;
    let body = payload(line, limits.max_request_line_bytes)?;
    let value: DeclaredTargetRequestV1 =
        serde_json::from_slice(body).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    value
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(value)
}
pub fn decode_declared_target_response_line_v1(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<DeclaredTargetResponseV1, ProtocolCodecErrorV1> {
    let limits = clipped(limits)?;
    let body = payload(line, limits.max_response_line_bytes)?;
    let value: DeclaredTargetResponseV1 =
        serde_json::from_slice(body).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    value
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(value)
}
pub fn encode_declared_target_response_line_v1(
    response: &DeclaredTargetResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    let limits = clipped(limits)?;
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    encode(response, limits)
}

pub fn read_request_line_any_v4<R: BufRead>(
    reader: &mut R,
    limits: ProtocolLimitsV1,
) -> Result<Option<DebugRequestAnyV4>, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    // Mixed streams retain the old outer framing cap. The target decoder refuses
    // a target-family line above 4096 bytes before decoding its DTO.
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
    if probe.schema == DECLARED_TARGET_REQUEST_SCHEMA_V1 {
        decode_declared_target_request_line_v1(&line, limits)
            .map(DebugRequestAnyV4::DeclaredTargetV1)
            .map(Some)
    } else {
        crate::read_request_line_any_v3(&mut io::Cursor::new(&line), limits)
            .map(|value| value.map(DebugRequestAnyV4::Legacy))
    }
}
