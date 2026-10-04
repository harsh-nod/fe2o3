#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::process::ExitCode;

use fe2o3_semantic_import::MAX_PROFILER_BUNDLE_BYTES_V4;
use fe2o3_semantic_query::*;

const FRAME_BYTES: usize = 8;
const MAX_COMPARISON_BYTES: u64 = MAX_PROFILER_BUNDLE_BYTES_V4 * 2 + FRAME_BYTES as u64;

fn main() -> ExitCode {
    match run() {
        Ok(output) if std::io::stdout().lock().write_all(&output).is_ok() => ExitCode::SUCCESS,
        Ok(_) => fail("stdout_write"),
        Err(code) => fail(code),
    }
}

fn run() -> Result<Vec<u8>, &'static str> {
    let mut arguments = std::env::args_os();
    let _ = arguments.next();
    let command = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or("arguments")?;
    if arguments.next().is_some() {
        return Err("arguments");
    }
    let input = read_bounded()?;
    let (baseline, candidate) = split_frame(&input)?;
    match command.as_str() {
        "bundle-v4" => encode_profiler_bundle_comparison_v4(
            &compare_profiler_bundles_v4(baseline, candidate).map_err(|_| "comparison")?,
        ),
        "counter-delta-v2" => encode_profiler_numeric_comparison_v4(
            &compare_counter_values_v2(baseline, candidate).map_err(|_| "comparison")?,
        ),
        "pc-delta-v3" => encode_profiler_numeric_comparison_v4(
            &compare_pc_sample_counts_v3(baseline, candidate).map_err(|_| "comparison")?,
        ),
        _ => return Err("arguments"),
    }
    .map_err(|_| "encode")
}

fn split_frame(input: &[u8]) -> Result<(&[u8], &[u8]), &'static str> {
    if input.len() <= FRAME_BYTES {
        return Err("framing");
    }
    let baseline_len = usize::try_from(u64::from_le_bytes(
        input[..FRAME_BYTES].try_into().map_err(|_| "framing")?,
    ))
    .map_err(|_| "framing")?;
    let boundary = FRAME_BYTES.checked_add(baseline_len).ok_or("framing")?;
    if baseline_len == 0 || boundary >= input.len() {
        return Err("framing");
    }
    Ok((&input[FRAME_BYTES..boundary], &input[boundary..]))
}

fn read_bounded() -> Result<Vec<u8>, &'static str> {
    let max = usize::try_from(MAX_COMPARISON_BYTES).map_err(|_| "input_limit")?;
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut reader = std::io::stdin().lock();
    loop {
        if input.len() == max {
            if reader.read(&mut buffer[..1]).map_err(|_| "stdin_read")? != 0 {
                return Err("input_too_large");
            }
            break;
        }
        let read_limit = buffer.len().min(max - input.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .map_err(|_| "stdin_read")?;
        if read == 0 {
            break;
        }
        input.try_reserve_exact(read).map_err(|_| "allocation")?;
        if input.capacity() > max {
            return Err("allocation");
        }
        input.extend_from_slice(&buffer[..read]);
    }
    Ok(input)
}

fn fail(code: &'static str) -> ExitCode {
    let _ = writeln!(std::io::stderr().lock(), "{{\"error\":\"{code}\"}}");
    ExitCode::FAILURE
}
