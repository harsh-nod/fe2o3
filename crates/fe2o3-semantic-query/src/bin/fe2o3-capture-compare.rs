#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::process::ExitCode;

use fe2o3_semantic_query::*;

fn main() -> ExitCode {
    match run() {
        Ok(bytes) if std::io::stdout().lock().write_all(&bytes).is_ok() => ExitCode::SUCCESS,
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
    if input.len() < 8 {
        return Err("framing");
    }
    let baseline_len = usize::try_from(u64::from_le_bytes(
        input[..8].try_into().map_err(|_| "framing")?,
    ))
    .map_err(|_| "framing")?;
    let boundary = 8_usize.checked_add(baseline_len).ok_or("framing")?;
    if baseline_len == 0 || boundary >= input.len() {
        return Err("framing");
    }
    let baseline = &input[8..boundary];
    let candidate = &input[boundary..];
    let comparison = match command.as_str() {
        "dispatch-v1" => compare_dispatch_captures_v1(baseline, candidate),
        "counter-v2" => compare_counter_captures_v2(baseline, candidate),
        _ => return Err("arguments"),
    }
    .map_err(|_| "comparison")?;
    encode_capture_comparison_v1(&comparison).map_err(|_| "encode")
}

fn read_bounded() -> Result<Vec<u8>, &'static str> {
    let max = usize::try_from(MAX_COMPARISON_INPUT_BYTES_V1)
        .map_err(|_| "input_limit")?
        .checked_add(8)
        .ok_or("input_limit")?;
    let mut bytes = Vec::new();
    let mut reader = std::io::stdin().lock();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        if bytes.len() == max {
            if reader.read(&mut buffer[..1]).map_err(|_| "stdin_read")? != 0 {
                return Err("input_too_large");
            }
            break;
        }
        let read_limit = buffer.len().min(max - bytes.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .map_err(|_| "stdin_read")?;
        if read == 0 {
            break;
        }
        let required = bytes.len().checked_add(read).ok_or("input_limit")?;
        if required > bytes.capacity() {
            let target = required
                .max(bytes.capacity().max(1).saturating_mul(2))
                .min(max);
            bytes
                .try_reserve_exact(target - bytes.capacity())
                .map_err(|_| "allocation")?;
            if bytes.capacity() > max {
                return Err("allocation");
            }
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

fn fail(code: &'static str) -> ExitCode {
    let _ = writeln!(std::io::stderr().lock(), "{{\"error\":\"{code}\"}}");
    ExitCode::FAILURE
}
