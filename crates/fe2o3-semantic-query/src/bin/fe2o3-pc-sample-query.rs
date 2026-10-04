#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use fe2o3_semantic_import::CaptureIdentityV1;
use fe2o3_semantic_query::*;
use serde::Serialize;

const MAX_ARGUMENTS: usize = 20;
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

fn run() -> Result<Vec<u8>, CliErrorV3> {
    let request = parse_arguments()?;
    let limits = PcSampleQueryLimitsV3::default();
    let input = read_bounded_stdin(limits.max_input_bytes)?;
    let session = PcSampleQuerySessionV3::open(&input, limits).map_err(|_| {
        CliErrorV3::new(
            "capture_open",
            "stdin is not a canonical bounded Semantic PC Sample Capture V3 document",
        )
    })?;
    drop(input);
    session.query_json(request).map_err(|_| {
        CliErrorV3::new(
            "query",
            "the bounded read-only PC sample query was rejected",
        )
    })
}

fn parse_arguments() -> Result<PcSampleQueryRequestV3, CliErrorV3> {
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(MAX_ARGUMENTS)
        .map_err(|_| CliErrorV3::new("allocation", "could not reserve bounded arguments"))?;
    for argument in std::env::args_os().skip(1) {
        if arguments.len() == MAX_ARGUMENTS {
            return Err(CliErrorV3::new(
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
        .ok_or_else(|| CliErrorV3::new("arguments", usage()))?;
    if matches!(command, "capabilities" | "open") {
        if arguments.len() != 1 {
            return Err(CliErrorV3::new(
                "arguments",
                "this command accepts no options",
            ));
        }
        return Ok(if command == "capabilities" {
            PcSampleQueryRequestV3::Capabilities
        } else {
            PcSampleQueryRequestV3::Open
        });
    }
    if command == "inspect-dispatch" {
        if arguments.len() != 3 || arguments[1].to_str() != Some("--dispatch") {
            return Err(CliErrorV3::new(
                "arguments",
                "inspect-dispatch requires exactly --dispatch HEX",
            ));
        }
        return Ok(PcSampleQueryRequestV3::InspectDispatch {
            identity: parse_identity(arguments[2].to_str().expect("validated UTF-8"))?,
        });
    }
    let kind = match command {
        "list-dispatches" => PcSampleListKindV3::Dispatches,
        "list-samples" => PcSampleListKindV3::Samples,
        "pc-hotspots" => PcSampleListKindV3::PcHotspots,
        _ => return Err(CliErrorV3::new("arguments", usage())),
    };
    let mut page = PcSamplePageRequestV3::default();
    let mut position = 1;
    let mut seen = 0_u8;
    while position < arguments.len() {
        let flag = arguments[position].to_str().expect("validated UTF-8");
        let value = arguments
            .get(position + 1)
            .and_then(|value| value.to_str())
            .ok_or_else(|| CliErrorV3::new("arguments", "every option requires one value"))?;
        position += 2;
        let bit = match flag {
            "--limit" => 1,
            "--cursor" => 2,
            "--dispatch" => 4,
            "--code-object" => 8,
            _ => return Err(CliErrorV3::new("arguments", "unknown option")),
        };
        if seen & bit != 0 {
            return Err(CliErrorV3::new("arguments", "duplicate option"));
        }
        seen |= bit;
        match flag {
            "--limit" => {
                page.limit = value
                    .parse()
                    .map_err(|_| CliErrorV3::new("arguments", "limit must be u16"))?;
            }
            "--cursor" => page.cursor = Some(parse_cursor(value)?),
            "--dispatch" => page.dispatch_filter = Some(parse_identity(value)?),
            "--code-object" => page.code_object_filter = Some(parse_identity(value)?),
            _ => unreachable!(),
        }
    }
    Ok(PcSampleQueryRequestV3::List { kind, page })
}

fn parse_cursor(value: &str) -> Result<PcSampleCursorV3, CliErrorV3> {
    let (binding, position) = value
        .split_once(':')
        .ok_or_else(|| CliErrorV3::new("cursor", "cursor must be QUERY_BINDING_HEX:POSITION"))?;
    Ok(PcSampleCursorV3 {
        query_binding: parse_identity(binding)?,
        position: position
            .parse()
            .map_err(|_| CliErrorV3::new("cursor", "cursor position must be u64"))?,
    })
}

fn parse_identity(value: &str) -> Result<CaptureIdentityV1, CliErrorV3> {
    if value.len() != 64 {
        return Err(CliErrorV3::new(
            "identity",
            "identity must be 64 lowercase hex digits",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    CaptureIdentityV1::new(bytes)
        .map_err(|_| CliErrorV3::new("identity", "identity cannot be all zero"))
}

fn nibble(value: u8) -> Result<u8, CliErrorV3> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(CliErrorV3::new(
            "identity",
            "identity must be lowercase hexadecimal",
        )),
    }
}

fn read_bounded_stdin(max: u64) -> Result<Vec<u8>, CliErrorV3> {
    let max = usize::try_from(max)
        .map_err(|_| CliErrorV3::new("input_limit", "input limit is not representable"))?;
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut reader = std::io::stdin().lock();
    loop {
        if input.len() == max {
            if reader
                .read(&mut buffer[..1])
                .map_err(|_| CliErrorV3::new("stdin_read", "could not read bounded stdin"))?
                != 0
            {
                return Err(CliErrorV3::new(
                    "input_too_large",
                    "stdin exceeds PC sample capture input limit",
                ));
            }
            break;
        }
        let read_limit = buffer.len().min(max - input.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .map_err(|_| CliErrorV3::new("stdin_read", "could not read bounded stdin"))?;
        if read == 0 {
            break;
        }
        let required = input
            .len()
            .checked_add(read)
            .ok_or_else(|| CliErrorV3::new("input_limit", "stdin size overflowed"))?;
        if required > input.capacity() {
            let target = required
                .max(input.capacity().max(1).saturating_mul(2))
                .min(max);
            input
                .try_reserve_exact(target - input.capacity())
                .map_err(|_| CliErrorV3::new("allocation", "could not grow bounded stdin"))?;
            if input.capacity() > max {
                return Err(CliErrorV3::new(
                    "allocation",
                    "stdin allocation exceeded its configured bound",
                ));
            }
        }
        input.extend_from_slice(&buffer[..read]);
    }
    Ok(input)
}

fn validate_argument(argument: &OsString) -> Result<(), CliErrorV3> {
    if argument.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES
        || argument.to_str().is_none()
    {
        return Err(CliErrorV3::new(
            "arguments",
            "arguments must be bounded UTF-8",
        ));
    }
    Ok(())
}

const fn usage() -> &'static str {
    "usage: fe2o3-pc-sample-query {capabilities|open|list-dispatches|list-samples|pc-hotspots|inspect-dispatch} [--limit N] [--cursor HEX:N] [--dispatch HEX] [--code-object HEX]"
}

#[derive(Debug)]
struct CliErrorV3 {
    code: &'static str,
    message: String,
}

impl CliErrorV3 {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct CliErrorResponseV3<'a> {
    error: &'a str,
    message: &'a str,
}

fn emit_error(code: &'static str, message: impl AsRef<str>) -> ExitCode {
    let message = message.as_ref();
    let response = CliErrorResponseV3 {
        error: code,
        message,
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &response);
    let _ = stderr.write_all(b"\n");
    ExitCode::FAILURE
}
