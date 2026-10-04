use std::fmt;
use std::io::{self, BufRead, Write};

use crate::{DebugRequestV1, DebugResponseV1, ProtocolLimitsV1, ProtocolValidationErrorV1};

/// Decodes one complete newline-terminated request frame.
pub fn decode_request_line_v1(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<DebugRequestV1, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line(line, limits.max_request_line_bytes)?;
    let request: DebugRequestV1 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    request
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(request)
}

/// Decodes one complete response frame for protocol conformance tests and clients.
pub fn decode_response_line_v1(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<DebugResponseV1, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line(line, limits.max_response_line_bytes)?;
    let response: DebugResponseV1 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(response)
}

/// Reads exactly one bounded request frame from a buffered stream.
///
/// The function returns `Ok(None)` only for clean EOF between frames. EOF after
/// any byte is a typed missing-terminator error. It never consumes bytes beyond
/// the first newline, so callers can process an arbitrary stream one request at
/// a time without accumulating more than one bounded line.
pub fn read_request_line_v1<R: BufRead>(
    reader: &mut R,
    limits: ProtocolLimitsV1,
) -> Result<Option<DebugRequestV1>, ProtocolCodecErrorV1> {
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
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |position| position + 1);
        append_bounded(
            &mut line,
            &available[..consumed],
            limits.max_request_line_bytes,
        )?;
        reader.consume(consumed);
        if newline.is_some() {
            return decode_request_line_v1(&line, limits).map(Some);
        }
    }
}

/// Encodes one response as compact JSON followed by exactly one newline.
pub fn encode_response_line_v1(
    response: &DebugResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload_limit = limits
        .max_response_line_bytes
        .checked_sub(1)
        .ok_or(ProtocolCodecErrorV1::ResponseTooLarge)?;
    let mut output = Vec::new();
    let mut writer = BoundedWriterV1 {
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
    reserve_exact_bounded(&mut output, 1, limits.max_response_line_bytes)?;
    output.push(b'\n');
    Ok(output)
}

fn validate_line(line: &[u8], max: usize) -> Result<&[u8], ProtocolCodecErrorV1> {
    if line.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if line.len() > max {
        return Err(ProtocolCodecErrorV1::LineTooLarge);
    }
    let Some(payload) = line.strip_suffix(b"\n") else {
        return Err(ProtocolCodecErrorV1::MissingLineTerminator);
    };
    if payload.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if payload.iter().any(|byte| matches!(*byte, b'\n' | b'\r')) {
        return Err(ProtocolCodecErrorV1::EmbeddedLineBreak);
    }
    Ok(payload)
}

fn append_bounded(
    output: &mut Vec<u8>,
    bytes: &[u8],
    max: usize,
) -> Result<(), ProtocolCodecErrorV1> {
    reserve_exact_bounded(output, bytes.len(), max)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn reserve_exact_bounded(
    output: &mut Vec<u8>,
    additional: usize,
    max: usize,
) -> Result<(), ProtocolCodecErrorV1> {
    let required = output
        .len()
        .checked_add(additional)
        .ok_or(ProtocolCodecErrorV1::LineTooLarge)?;
    if required > max {
        return Err(ProtocolCodecErrorV1::LineTooLarge);
    }
    if required > output.capacity() {
        output
            .try_reserve_exact(required - output.capacity())
            .map_err(|_| ProtocolCodecErrorV1::AllocationFailure)?;
    }
    Ok(())
}

struct BoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    limit_exceeded: bool,
    allocation_failed: bool,
}

impl Write for BoundedWriterV1<'_> {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolCodecErrorV1 {
    Validation(ProtocolValidationErrorV1),
    EmptyLine,
    LineTooLarge,
    MissingLineTerminator,
    EmbeddedLineBreak,
    InvalidJson,
    JsonEncode,
    InputRead,
    AllocationFailure,
    ResponseTooLarge,
}

impl ProtocolCodecErrorV1 {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Validation(_) => "invalid_protocol_value",
            Self::EmptyLine => "empty_line",
            Self::LineTooLarge => "line_too_large",
            Self::MissingLineTerminator => "missing_line_terminator",
            Self::EmbeddedLineBreak => "embedded_line_break",
            Self::InvalidJson => "invalid_json",
            Self::JsonEncode => "json_encode",
            Self::InputRead => "input_read",
            Self::AllocationFailure => "allocation_failure",
            Self::ResponseTooLarge => "response_too_large",
        }
    }
}

impl fmt::Display for ProtocolCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "fe2o3 debug protocol codec failed: {self:?}")
    }
}

impl std::error::Error for ProtocolCodecErrorV1 {}
