#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use fe2o3_semantic_query::*;
use serde::Serialize;

const MAX_ARGUMENTS: usize = 64;
const MAX_ARGUMENT_BYTES: usize = 256;

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            if std::io::stdout().lock().write_all(&output).is_ok() {
                ExitCode::SUCCESS
            } else {
                emit_error(
                    "stdout_write",
                    "could not publish the complete bounded response",
                )
            }
        }
        Err(error) => emit_error(error.code, error.message),
    }
}

fn run() -> Result<Vec<u8>, CliErrorV1> {
    let request = parse_arguments()?;
    let limits = QueryLimitsV1::default();
    let input = read_bounded_stdin(limits.max_input_bytes())?;
    let session = TraceQuerySessionV1::open(&input, limits).map_err(|_| {
        CliErrorV1::new(
            "trace_open",
            "stdin is not a valid bounded canonical Trace V1 stream",
        )
    })?;
    drop(input);
    session
        .query_json(request)
        .map_err(|_| CliErrorV1::new("query", "the bounded semantic query was rejected"))
}

fn read_bounded_stdin(max: u64) -> Result<Vec<u8>, CliErrorV1> {
    let max = usize::try_from(max).map_err(|_| {
        CliErrorV1::new(
            "input_limit",
            "the configured input limit cannot be represented",
        )
    })?;
    let mut input = Vec::new();
    let mut reader = std::io::stdin().lock();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        if input.len() == max {
            if reader.read(&mut buffer[..1]).map_err(|_| {
                CliErrorV1::new("stdin_read", "could not read the bounded stdin stream")
            })? != 0
            {
                return Err(CliErrorV1::new(
                    "input_too_large",
                    "stdin exceeds the maximum Trace V1 input size",
                ));
            }
            break;
        }
        let remaining = max - input.len();
        let read_limit = remaining.min(buffer.len());
        let read = reader.read(&mut buffer[..read_limit]).map_err(|_| {
            CliErrorV1::new("stdin_read", "could not read the bounded stdin stream")
        })?;
        if read == 0 {
            break;
        }
        reserve_bounded_input(&mut input, read, max)?;
        input.extend_from_slice(&buffer[..read]);
    }
    Ok(input)
}

fn reserve_bounded_input(
    input: &mut Vec<u8>,
    additional: usize,
    max: usize,
) -> Result<(), CliErrorV1> {
    let required = input
        .len()
        .checked_add(additional)
        .ok_or_else(|| CliErrorV1::new("input_too_large", "stdin size arithmetic overflowed"))?;
    if required > max {
        return Err(CliErrorV1::new(
            "input_too_large",
            "stdin exceeds the maximum Trace V1 input size",
        ));
    }
    if required <= input.capacity() {
        return Ok(());
    }
    let doubled = input.capacity().checked_mul(2).unwrap_or(max);
    let target = required.max(doubled).min(max);
    input
        .try_reserve_exact(target.saturating_sub(input.capacity()))
        .map_err(|_| CliErrorV1::new("allocation", "could not grow the bounded input buffer"))?;
    if input.capacity() > max {
        return Err(CliErrorV1::new(
            "allocation",
            "the input allocator exceeded the configured resident bound",
        ));
    }
    Ok(())
}

fn parse_arguments() -> Result<QueryRequestV1, CliErrorV1> {
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(MAX_ARGUMENTS)
        .map_err(|_| CliErrorV1::new("allocation", "could not reserve the bounded argument set"))?;
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
    let Some(command) = arguments.first().and_then(|value| value.to_str()) else {
        return Err(CliErrorV1::new("arguments", usage()));
    };
    if command == "capabilities" || command == "summary" {
        if arguments.len() != 1 {
            return Err(CliErrorV1::new(
                "arguments",
                "this command does not accept filters",
            ));
        }
        return Ok(if command == "capabilities" {
            QueryRequestV1::Capabilities
        } else {
            QueryRequestV1::DispatchSummary
        });
    }
    if command == "plan-next-capture" || command == "diagnosis-status" {
        if arguments.len() != 3
            || arguments.get(1).and_then(|value| value.to_str()) != Some("--goal")
        {
            return Err(CliErrorV1::new(
                "arguments",
                "this command requires exactly --goal GOAL",
            ));
        }
        let goal = parse_capture_goal(
            arguments[2]
                .to_str()
                .ok_or_else(|| CliErrorV1::new("arguments", "goal must be valid UTF-8"))?,
        )?;
        return Ok(if command == "plan-next-capture" {
            QueryRequestV1::PlanNextCapture { goal }
        } else {
            QueryRequestV1::DiagnosisStatus { goal }
        });
    }
    let kind = parse_page_kind(command)?;
    let mut page = PageRequestV1::default();
    let mut filter = QueryFilterV1::default();
    let mut seen_flags = 0_u16;
    let mut position = 1;
    while position < arguments.len() {
        let flag = arguments[position]
            .to_str()
            .ok_or_else(|| CliErrorV1::new("arguments", "arguments must be valid UTF-8"))?;
        position += 1;
        let value = arguments
            .get(position)
            .and_then(|value| value.to_str())
            .ok_or_else(|| CliErrorV1::new("arguments", "every filter flag requires one value"))?;
        position += 1;
        let flag_bit = flag_bit(flag)?;
        if seen_flags & flag_bit != 0 {
            return Err(CliErrorV1::new(
                "arguments",
                "each query filter flag may appear at most once",
            ));
        }
        seen_flags |= flag_bit;
        match flag {
            "--limit" => page.limit = parse_u16(value)?,
            "--cursor" => page.cursor = Some(parse_cursor(value)?),
            "--sequence-start" => filter.sequence_start = Some(parse_u64(value)?),
            "--sequence-end" => filter.sequence_end = Some(parse_u64(value)?),
            "--workgroup" => filter.workgroup = Some(parse_u32x3(value)?),
            "--wave" => filter.wave = Some(parse_u32(value)?),
            "--lane" => filter.lane = Some(parse_u16(value)?),
            "--function" => filter.function_ordinal = Some(parse_u64(value)?),
            "--block" => filter.block_ordinal = Some(parse_u64(value)?),
            "--operation" => filter.operation_ordinal = Some(parse_u64(value)?),
            "--allocation" => filter.allocation = Some(parse_u64x2(value)?),
            "--memory-access" => filter.memory_access = Some(parse_memory_access(value)?),
            "--provenance" => filter.provenance = Some(parse_provenance(value)?),
            "--evidence-kind" => filter.evidence_kind = Some(parse_evidence_kind(value)?),
            _ => return Err(CliErrorV1::new("arguments", "unknown query filter flag")),
        }
    }
    Ok(QueryRequestV1::Page { kind, page, filter })
}

fn validate_argument(argument: &OsString) -> Result<(), CliErrorV1> {
    let bytes = argument.as_os_str().as_encoded_bytes();
    if bytes.len() > MAX_ARGUMENT_BYTES {
        return Err(CliErrorV1::new(
            "arguments",
            "one command-line argument is too long",
        ));
    }
    if argument.to_str().is_none() {
        return Err(CliErrorV1::new(
            "arguments",
            "arguments must be valid UTF-8",
        ));
    }
    Ok(())
}

fn flag_bit(flag: &str) -> Result<u16, CliErrorV1> {
    let bit = match flag {
        "--limit" => 0,
        "--cursor" => 1,
        "--sequence-start" => 2,
        "--sequence-end" => 3,
        "--workgroup" => 4,
        "--wave" => 5,
        "--lane" => 6,
        "--function" => 7,
        "--block" => 8,
        "--operation" => 9,
        "--allocation" => 10,
        "--memory-access" => 11,
        "--provenance" => 12,
        "--evidence-kind" => 13,
        _ => return Err(CliErrorV1::new("arguments", "unknown query filter flag")),
    };
    Ok(1_u16 << bit)
}

fn parse_page_kind(value: &str) -> Result<PageKindV1, CliErrorV1> {
    match value {
        "workgroups" => Ok(PageKindV1::Workgroups),
        "waves" => Ok(PageKindV1::Waves),
        "lanes" => Ok(PageKindV1::Lanes),
        "sites" => Ok(PageKindV1::Sites),
        "occurrences" => Ok(PageKindV1::OperationOccurrences),
        "memory-accesses" => Ok(PageKindV1::MemoryAccesses),
        "memory-regions" => Ok(PageKindV1::MemoryRegions),
        "faults" => Ok(PageKindV1::Faults),
        "evidence" => Ok(PageKindV1::ProvenanceAndEvidence),
        _ => Err(CliErrorV1::new("arguments", usage())),
    }
}

fn parse_capture_goal(value: &str) -> Result<CaptureGoalV1, CliErrorV1> {
    match value {
        "memory_fault" => Ok(CaptureGoalV1::MemoryFault),
        "barrier_divergence" => Ok(CaptureGoalV1::BarrierDivergence),
        "performance_hotspot" => Ok(CaptureGoalV1::PerformanceHotspot),
        "correctness_mismatch" => Ok(CaptureGoalV1::CorrectnessMismatch),
        _ => Err(CliErrorV1::new(
            "arguments",
            "goal must be memory_fault, barrier_divergence, performance_hotspot, or correctness_mismatch",
        )),
    }
}

fn parse_cursor(value: &str) -> Result<QueryCursorV1, CliErrorV1> {
    let (identity, position) = value.split_once(':').ok_or_else(|| {
        CliErrorV1::new(
            "cursor",
            "cursor must be 64 lowercase hex digits followed by :POSITION",
        )
    })?;
    if identity.len() != 64
        || !identity
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CliErrorV1::new(
            "cursor",
            "cursor identity must contain exactly 64 hex digits",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in identity.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(QueryCursorV1 {
        query_binding: OpaqueIdentityViewV1 { bytes },
        event_position: parse_u64(position)?,
    })
}

fn hex_nibble(byte: u8) -> Result<u8, CliErrorV1> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(CliErrorV1::new(
            "cursor",
            "cursor identity contains non-hex data",
        )),
    }
}

fn parse_u16(value: &str) -> Result<u16, CliErrorV1> {
    value
        .parse()
        .map_err(|_| CliErrorV1::new("arguments", "expected an unsigned 16-bit integer"))
}

fn parse_u32(value: &str) -> Result<u32, CliErrorV1> {
    value
        .parse()
        .map_err(|_| CliErrorV1::new("arguments", "expected an unsigned 32-bit integer"))
}

fn parse_u64(value: &str) -> Result<u64, CliErrorV1> {
    value
        .parse()
        .map_err(|_| CliErrorV1::new("arguments", "expected an unsigned 64-bit integer"))
}

fn parse_u32x3(value: &str) -> Result<[u32; 3], CliErrorV1> {
    let mut parts = value.split(',');
    let output = [
        parse_u32(parts.next().unwrap_or(""))?,
        parse_u32(parts.next().unwrap_or(""))?,
        parse_u32(parts.next().unwrap_or(""))?,
    ];
    if parts.next().is_some() {
        return Err(CliErrorV1::new(
            "arguments",
            "expected three comma-separated coordinates",
        ));
    }
    Ok(output)
}

fn parse_u64x2(value: &str) -> Result<(u64, u64), CliErrorV1> {
    let mut parts = value.split(',');
    let output = (
        parse_u64(parts.next().unwrap_or(""))?,
        parse_u64(parts.next().unwrap_or(""))?,
    );
    if parts.next().is_some() {
        return Err(CliErrorV1::new(
            "arguments",
            "expected allocation ordinal,generation",
        ));
    }
    Ok(output)
}

fn parse_memory_access(value: &str) -> Result<MemoryAccessFilterV1, CliErrorV1> {
    match value {
        "read" => Ok(MemoryAccessFilterV1::Read),
        "write" => Ok(MemoryAccessFilterV1::Write),
        "atomic" => Ok(MemoryAccessFilterV1::Atomic),
        _ => Err(CliErrorV1::new(
            "arguments",
            "memory access must be read, write, or atomic",
        )),
    }
}

fn parse_provenance(value: &str) -> Result<ProvenanceFilterV1, CliErrorV1> {
    match value {
        "declared" => Ok(ProvenanceFilterV1::Declared),
        "proved" => Ok(ProvenanceFilterV1::Proved),
        "observed" => Ok(ProvenanceFilterV1::Observed),
        "inferred" => Ok(ProvenanceFilterV1::Inferred),
        "unavailable" => Ok(ProvenanceFilterV1::Unavailable),
        _ => Err(CliErrorV1::new("arguments", "unknown provenance filter")),
    }
}

fn parse_evidence_kind(value: &str) -> Result<EvidenceKindFilterV1, CliErrorV1> {
    match value {
        "declaration" => Ok(EvidenceKindFilterV1::Declaration),
        "proof" => Ok(EvidenceKindFilterV1::Proof),
        "inference-rule" => Ok(EvidenceKindFilterV1::InferenceRule),
        "runtime-observation" => Ok(EvidenceKindFilterV1::RuntimeObservation),
        "artifact" => Ok(EvidenceKindFilterV1::Artifact),
        _ => Err(CliErrorV1::new("arguments", "unknown evidence-kind filter")),
    }
}

const fn usage() -> &'static str {
    "usage: fe2o3-trace-query COMMAND [--goal GOAL|FILTER VALUE ...] < canonical-trace-v1"
}

#[derive(Debug)]
struct CliErrorV1 {
    code: &'static str,
    message: &'static str,
}

impl CliErrorV1 {
    const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
}

#[derive(Serialize)]
struct CliErrorResponseV1 {
    schema: &'static str,
    response: &'static str,
    code: &'static str,
    message: &'static str,
}

fn emit_error(code: &'static str, message: &'static str) -> ExitCode {
    let response = CliErrorResponseV1 {
        schema: QUERY_SCHEMA_V1,
        response: "error",
        code,
        message,
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &response);
    let _ = stderr.write_all(b"\n");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_growth_is_proportional_and_never_crosses_limit() {
        let mut input = Vec::new();
        reserve_bounded_input(&mut input, 1, 16).unwrap();
        input.push(1);
        assert!(input.capacity() <= 16);
        reserve_bounded_input(&mut input, 7, 16).unwrap();
        input.extend_from_slice(&[2; 7]);
        assert!(input.capacity() <= 16);
        let error = reserve_bounded_input(&mut input, 9, 16).unwrap_err();
        assert_eq!(error.code, "input_too_large");
        assert_eq!(input.len(), 8);
        assert!(input.capacity() <= 16);
    }
}
