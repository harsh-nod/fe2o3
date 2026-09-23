use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

#[path = "common.rs"]
mod common;
#[path = "loop_cases.rs"]
mod loop_cases;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[path = "../../../fe2o3-kir-sim/examples/runtime_origin_source_v1/topology.rs"]
mod topology;
#[path = "workgroup_cases.rs"]
mod workgroup_cases;
#[path = "workgroup_inspection.rs"]
mod workgroup_inspection;
use common::*;

fn arguments(args: Vec<String>) -> Result<(String, String, String), String> {
    if args.len() != 3
        || !matches!(args[0].as_str(), "loop" | "workgroup" | "inspect-workgroup")
        || !Path::new(&args[1]).is_absolute()
        || args[1].len() > 4096
        || args[1].chars().any(char::is_control)
        || args[2].len() != 64
        || !args[2]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("usage: observe_runtime_observations_source_v1 loop|workgroup|inspect-workgroup ABSOLUTE_BUNDLE EXPECTED_SHA256".into());
    }
    Ok((args[0].clone(), args[1].clone(), args[2].clone()))
}
fn read_bundle(path: &str, expected: &str) -> Result<Vec<u8>, String> {
    let before = std::fs::symlink_metadata(path).map_err(fail)?;
    demand(
        before.is_file() && before.len() > 0 && before.len() <= BUNDLE_CAP as u64,
        "bundle cap/type",
    )?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(fail)?;
    let opened = file.metadata().map_err(fail)?;
    demand(
        opened.is_file()
            && opened.dev() == before.dev()
            && opened.ino() == before.ino()
            && opened.len() == before.len(),
        "bundle identity changed before open",
    )?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(BUNDLE_CAP as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(fail)?;
    let after = file.metadata().map_err(fail)?;
    demand(
        after.len() == opened.len()
            && after.mtime_nsec() == opened.mtime_nsec()
            && after.mtime() == opened.mtime()
            && after.ctime_nsec() == opened.ctime_nsec()
            && after.ctime() == opened.ctime(),
        "bundle changed during read",
    )?;
    demand(
        bytes.len() == opened.len() as usize && hash(&bytes) == expected,
        "exact bounded bundle byte hash",
    )?;
    Ok(bytes)
}
fn report(mode: &str, bytes: Vec<u8>) -> Result<Value, String> {
    if mode == "inspect-workgroup" {
        return workgroup_inspection::inspect(bytes);
    }
    let result = match mode {
        "loop" => loop_cases::observe(bytes)?,
        "workgroup" => workgroup_cases::observe(bytes)?,
        _ => return Err("unknown mode".into()),
    };
    Ok(
        json!({"schema":"task-runtime-observations-source-observer-v1","status":"passed","mode":mode,
        "source_authenticated":false,"hardware_observed":false,"compiler_resume_authority":false,
        "scope":"fresh ordinary Rust export, retained CPU frame/allocator observations; not wire/browser qualification",
        "profile":{"records":65536,"origin_bytes":4194304,"frame_rows":131072,"frame_bytes":25165824,
            "lifecycle_transitions":8192,"lifecycle_bytes":8388608,"lifecycle_validation_work":1000000,"reuse_bytes":8192},
        "result":result}),
    )
}
fn execute() -> Result<(), String> {
    let (mode, path, expected) = arguments(std::env::args().skip(1).collect())?;
    let bytes = read_bundle(&path, &expected)?;
    let encoded = serde_json::to_vec(&report(&mode, bytes)?).map_err(fail)?;
    demand(encoded.len() <= REPORT_CAP, "cumulative report cap")?;
    let mut output = std::io::stdout().lock();
    output.write_all(&encoded).map_err(fail)?;
    output.write_all(b"\n").map_err(fail)
}
pub(super) fn main() -> std::process::ExitCode {
    match execute() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "runtime observation source acceptance refused: {}",
                error.chars().take(2048).collect::<String>()
            );
            std::process::ExitCode::FAILURE
        }
    }
}
