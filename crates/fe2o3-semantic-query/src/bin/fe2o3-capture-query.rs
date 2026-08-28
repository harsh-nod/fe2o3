#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use fe2o3_semantic_import::CaptureIdentityV1;
use fe2o3_semantic_query::*;
use serde::Serialize;

const MAX_ARGUMENTS: usize = 16;
const MAX_ARGUMENT_BYTES: usize = 256;

fn main() -> ExitCode {
    match run() {
        Ok(output) if std::io::stdout().lock().write_all(&output).is_ok() => ExitCode::SUCCESS,
        Ok(_) => emit_error(
            "stdout_write",
            "could not publish the complete bounded response",
        ),
        Err(error) => emit_error(error.code, error.message),
    }
}

fn run() -> Result<Vec<u8>, CliErrorV1> {
    let request = parse_arguments()?;
    let limits = CaptureQueryLimitsV1::default();
    let input = read_bounded_stdin(limits.max_input_bytes())?;
    let session = CaptureQuerySessionV1::open(&input, limits).map_err(|_| {
        CliErrorV1::new(
            "capture_open",
            "stdin is not a canonical bounded Semantic Capture V1 document",
        )
    })?;
    drop(input);
    session
        .query_json(request)
        .map_err(|_| CliErrorV1::new("query", "the bounded read-only capture query was rejected"))
}

fn parse_arguments() -> Result<CaptureQueryRequestV1, CliErrorV1> {
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(MAX_ARGUMENTS)
        .map_err(|_| CliErrorV1::new("allocation", "could not reserve bounded arguments"))?;
    for argument in std::env::args_os().skip(1) {
        if arguments.len() == MAX_ARGUMENTS {
            return Err(CliErrorV1::new(
                "arguments",
                "too many command-line arguments",
            ));
        }
        validate_argument(&argument)?;
        arguments.push(argument);
    }
    let command = arguments
        .first()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CliErrorV1::new("arguments", usage()))?;
    if matches!(command, "capabilities" | "open") {
        if arguments.len() != 1 {
            return Err(CliErrorV1::new(
                "arguments",
                "this command accepts no options",
            ));
        }
        return Ok(if command == "capabilities" {
            CaptureQueryRequestV1::Capabilities
        } else {
            CaptureQueryRequestV1::Open
        });
    }
    if command == "inspect-dispatch" {
        if arguments.len() != 3 || arguments[1].to_str() != Some("--dispatch") {
            return Err(CliErrorV1::new(
                "arguments",
                "inspect-dispatch requires exactly --dispatch HEX",
            ));
        }
        return Ok(CaptureQueryRequestV1::InspectDispatch {
            identity: parse_identity(arguments[2].to_str().expect("validated UTF-8"))?,
        });
    }
    let kind = match command {
        "list-runs" => CaptureListKindV1::Runs,
        "list-devices" => CaptureListKindV1::Devices,
        "list-dispatches" => CaptureListKindV1::Dispatches,
        "hotspots" => CaptureListKindV1::Hotspots,
        _ => return Err(CliErrorV1::new("arguments", usage())),
    };
    let mut page = CapturePageRequestV1::default();
    let mut position = 1;
    let mut seen = 0_u8;
    while position < arguments.len() {
        let flag = arguments[position].to_str().expect("validated UTF-8");
        let value = arguments
            .get(position + 1)
            .and_then(|value| value.to_str())
            .ok_or_else(|| CliErrorV1::new("arguments", "every option requires one value"))?;
        position += 2;
        let bit = match flag {
            "--limit" => 1,
            "--cursor" => 2,
            _ => return Err(CliErrorV1::new("arguments", "unknown option")),
        };
        if seen & bit != 0 {
            return Err(CliErrorV1::new("arguments", "duplicate option"));
        }
        seen |= bit;
        match flag {
            "--limit" => {
                page.limit = value
                    .parse()
                    .map_err(|_| CliErrorV1::new("arguments", "limit must be u16"))?
            }
            "--cursor" => page.cursor = Some(parse_cursor(value)?),
            _ => unreachable!(),
        }
    }
    Ok(CaptureQueryRequestV1::List { kind, page })
}

fn parse_cursor(value: &str) -> Result<CaptureCursorV1, CliErrorV1> {
    let (binding, position) = value
        .split_once(':')
        .ok_or_else(|| CliErrorV1::new("cursor", "cursor must be QUERY_BINDING_HEX:POSITION"))?;
    Ok(CaptureCursorV1::new(
        parse_identity(binding)?,
        position
            .parse()
            .map_err(|_| CliErrorV1::new("cursor", "cursor position must be u64"))?,
    ))
}

fn parse_identity(value: &str) -> Result<CaptureIdentityV1, CliErrorV1> {
    if value.len() != 64 {
        return Err(CliErrorV1::new(
            "identity",
            "identity must be 64 lowercase hex digits",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    CaptureIdentityV1::new(bytes)
        .map_err(|_| CliErrorV1::new("identity", "identity cannot be all zero"))
}

fn nibble(value: u8) -> Result<u8, CliErrorV1> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(CliErrorV1::new(
            "identity",
            "identity must be lowercase hexadecimal",
        )),
    }
}

fn read_bounded_stdin(max: u64) -> Result<Vec<u8>, CliErrorV1> {
    let max = usize::try_from(max)
        .map_err(|_| CliErrorV1::new("input_limit", "input limit is not representable"))?;
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut reader = std::io::stdin().lock();
    loop {
        if input.len() == max {
            if reader
                .read(&mut buffer[..1])
                .map_err(|_| CliErrorV1::new("stdin_read", "could not read bounded stdin"))?
                != 0
            {
                return Err(CliErrorV1::new(
                    "input_too_large",
                    "stdin exceeds capture input limit",
                ));
            }
            break;
        }
        let read_limit = buffer.len().min(max - input.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .map_err(|_| CliErrorV1::new("stdin_read", "could not read bounded stdin"))?;
        if read == 0 {
            break;
        }
        input
            .try_reserve_exact(read)
            .map_err(|_| CliErrorV1::new("allocation", "could not grow bounded stdin"))?;
        if input.capacity() > max {
            return Err(CliErrorV1::new(
                "allocation",
                "stdin allocation exceeded its configured bound",
            ));
        }
        input.extend_from_slice(&buffer[..read]);
    }
    Ok(input)
}

fn validate_argument(argument: &OsString) -> Result<(), CliErrorV1> {
    if argument.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES
        || argument.to_str().is_none()
    {
        return Err(CliErrorV1::new(
            "arguments",
            "arguments must be bounded UTF-8",
        ));
    }
    Ok(())
}

const fn usage() -> &'static str {
    "usage: fe2o3-capture-query {capabilities|open|list-runs|list-devices|list-dispatches|hotspots|inspect-dispatch} [--limit N] [--cursor HEX:N]"
}

#[derive(Debug)]
struct CliErrorV1 {
    code: &'static str,
    message: String,
}

impl CliErrorV1 {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct CliErrorResponseV1<'a> {
    error: &'a str,
    message: &'a str,
}

fn emit_error(code: &'static str, message: impl AsRef<str>) -> ExitCode {
    let message = message.as_ref();
    let response = CliErrorResponseV1 {
        error: code,
        message,
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &response);
    let _ = stderr.write_all(b"\n");
    ExitCode::FAILURE
}
