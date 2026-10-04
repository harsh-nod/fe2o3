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
        Ok(_) => fail(
            "stdout_write",
            "could not publish the complete bounded response",
        ),
        Err(error) => fail(error.code, error.message),
    }
}

fn run() -> Result<Vec<u8>, CliErrorV4> {
    let request = parse_arguments()?;
    let limits = ProfilerQueryLimitsV4::default();
    let input = read_bounded(limits.max_input_bytes)?;
    let session = ProfilerQuerySessionV4::open(&input, limits).map_err(|_| {
        CliErrorV4::new(
            "bundle_open",
            "stdin is not a canonical bounded Semantic Profiler Bundle V4",
        )
    })?;
    drop(input);
    let response = session
        .query(request)
        .map_err(|_| CliErrorV4::new("query", "bounded read-only profiler query rejected"))?;
    session
        .encode_response(&response)
        .map_err(|_| CliErrorV4::new("encode", "bounded profiler response rejected"))
}

fn parse_arguments() -> Result<ProfilerQueryRequestV4, CliErrorV4> {
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(MAX_ARGUMENTS)
        .map_err(|_| CliErrorV4::new("allocation", "could not reserve bounded arguments"))?;
    for argument in std::env::args_os().skip(1) {
        validate_argument(&argument)?;
        if arguments.len() == MAX_ARGUMENTS {
            return Err(CliErrorV4::new("arguments", "too many arguments"));
        }
        arguments.push(argument);
    }
    let command = arguments
        .first()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CliErrorV4::new("arguments", usage()))?;
    if matches!(command, "capabilities" | "open") {
        exact_argument_count(&arguments, 1)?;
        return Ok(if command == "capabilities" {
            ProfilerQueryRequestV4::Capabilities
        } else {
            ProfilerQueryRequestV4::Open
        });
    }
    if command == "inspect-dispatch" {
        if arguments.len() != 3 || arguments[1].to_str() != Some("--dispatch") {
            return Err(CliErrorV4::new(
                "arguments",
                "inspect-dispatch requires exactly --dispatch HEX",
            ));
        }
        return Ok(ProfilerQueryRequestV4::InspectDispatch {
            identity: parse_identity(arguments[2].to_str().expect("validated UTF-8"))?,
        });
    }
    if let Some(goal) = match command {
        "plan-waits" => Some(ProfilerCaptureGoalV4::ExplainWaits),
        "plan-att" => Some(ProfilerCaptureGoalV4::DecodeAttCoverage),
        "plan-hotspots" => Some(ProfilerCaptureGoalV4::RankDispatchDurations),
        _ => None,
    } {
        exact_argument_count(&arguments, 1)?;
        return Ok(ProfilerQueryRequestV4::PlanNextCapture { goal });
    }
    let kind = match command {
        "list-runs" => ProfilerListKindV4::Runs,
        "list-devices" => ProfilerListKindV4::Devices,
        "list-dispatches" => ProfilerListKindV4::Dispatches,
        "hotspots" => ProfilerListKindV4::DurationHotspots,
        "list-att-references" => ProfilerListKindV4::AttReferences,
        "waits" => ProfilerListKindV4::Waits,
        _ => return Err(CliErrorV4::new("arguments", usage())),
    };
    let mut page = ProfilerPageRequestV4::default();
    let mut position = 1;
    let mut seen = 0_u8;
    while position < arguments.len() {
        let flag = arguments[position].to_str().expect("validated UTF-8");
        let value = arguments
            .get(position + 1)
            .and_then(|value| value.to_str())
            .ok_or_else(|| CliErrorV4::new("arguments", "every option requires one value"))?;
        position += 2;
        let bit = match flag {
            "--limit" => 1,
            "--cursor" => 2,
            _ => return Err(CliErrorV4::new("arguments", "unknown option")),
        };
        if seen & bit != 0 {
            return Err(CliErrorV4::new("arguments", "duplicate option"));
        }
        seen |= bit;
        match flag {
            "--limit" => {
                page.limit = value
                    .parse()
                    .map_err(|_| CliErrorV4::new("arguments", "limit must be u16"))?;
            }
            "--cursor" => page.cursor = Some(parse_cursor(value)?),
            _ => unreachable!(),
        }
    }
    Ok(ProfilerQueryRequestV4::List { kind, page })
}

fn exact_argument_count(arguments: &[OsString], expected: usize) -> Result<(), CliErrorV4> {
    if arguments.len() != expected {
        return Err(CliErrorV4::new(
            "arguments",
            "command accepts no additional arguments",
        ));
    }
    Ok(())
}

fn parse_cursor(value: &str) -> Result<ProfilerCursorV4, CliErrorV4> {
    let (binding, position) = value
        .split_once(':')
        .ok_or_else(|| CliErrorV4::new("cursor", "cursor must be QUERY_BINDING_HEX:POSITION"))?;
    Ok(ProfilerCursorV4 {
        query_binding: parse_identity(binding)?,
        position: position
            .parse()
            .map_err(|_| CliErrorV4::new("cursor", "cursor position must be u64"))?,
    })
}

fn parse_identity(value: &str) -> Result<CaptureIdentityV1, CliErrorV4> {
    if value.len() != 64 {
        return Err(CliErrorV4::new(
            "identity",
            "identity must be 64 lowercase hex digits",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    CaptureIdentityV1::new(bytes)
        .map_err(|_| CliErrorV4::new("identity", "identity cannot be all zero"))
}

fn nibble(value: u8) -> Result<u8, CliErrorV4> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(CliErrorV4::new(
            "identity",
            "identity must be lowercase hexadecimal",
        )),
    }
}

fn read_bounded(max: u64) -> Result<Vec<u8>, CliErrorV4> {
    let max = usize::try_from(max)
        .map_err(|_| CliErrorV4::new("input_limit", "input limit is not representable"))?;
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut reader = std::io::stdin().lock();
    loop {
        if input.len() == max {
            if reader
                .read(&mut buffer[..1])
                .map_err(|_| CliErrorV4::new("stdin_read", "could not read bounded stdin"))?
                != 0
            {
                return Err(CliErrorV4::new(
                    "input_too_large",
                    "stdin exceeds profiler bundle input limit",
                ));
            }
            break;
        }
        let read_limit = buffer.len().min(max - input.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .map_err(|_| CliErrorV4::new("stdin_read", "could not read bounded stdin"))?;
        if read == 0 {
            break;
        }
        input
            .try_reserve_exact(read)
            .map_err(|_| CliErrorV4::new("allocation", "could not grow bounded stdin"))?;
        if input.capacity() > max {
            return Err(CliErrorV4::new(
                "allocation",
                "stdin allocation exceeded configured bound",
            ));
        }
        input.extend_from_slice(&buffer[..read]);
    }
    Ok(input)
}

fn validate_argument(argument: &OsString) -> Result<(), CliErrorV4> {
    if argument.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES
        || argument.to_str().is_none()
    {
        return Err(CliErrorV4::new(
            "arguments",
            "arguments must be bounded UTF-8",
        ));
    }
    Ok(())
}

const fn usage() -> &'static str {
    "usage: fe2o3-profiler-query {capabilities|open|list-runs|list-devices|list-dispatches|hotspots|list-att-references|waits|inspect-dispatch|plan-waits|plan-att|plan-hotspots} [--limit N] [--cursor HEX:N]"
}

#[derive(Debug)]
struct CliErrorV4 {
    code: &'static str,
    message: String,
}

impl CliErrorV4 {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct CliErrorResponseV4<'a> {
    error: &'a str,
    message: &'a str,
}

fn fail(code: &'static str, message: impl AsRef<str>) -> ExitCode {
    let response = CliErrorResponseV4 {
        error: code,
        message: message.as_ref(),
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &response);
    let _ = stderr.write_all(b"\n");
    ExitCode::FAILURE
}
