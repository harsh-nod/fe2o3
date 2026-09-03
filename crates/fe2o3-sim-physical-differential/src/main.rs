use std::io::Write;
use std::process::ExitCode;

use serde::Serialize;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    if arguments
        .next()
        .as_deref()
        .and_then(std::ffi::OsStr::to_str)
        != Some("physical-capabilities-v1")
        || arguments.next().is_some()
    {
        return write_json_stderr(&CommandErrorV1 {
            schema: "fe2o3-sim-physical-differential-command-error-v1",
            code: "invalid_command_line",
            detail: "usage: fe2o3-sim-physical-differential physical-capabilities-v1",
            hardware_observed: false,
        });
    }
    match serde_json::to_vec(
        &fe2o3_sim_physical_differential::physical_differential_capabilities_v1(),
    ) {
        Ok(mut encoded) => {
            encoded.push(b'\n');
            if std::io::stdout().lock().write_all(&encoded).is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(_) => ExitCode::FAILURE,
    }
}

#[derive(Serialize)]
struct CommandErrorV1 {
    schema: &'static str,
    code: &'static str,
    detail: &'static str,
    hardware_observed: bool,
}

fn write_json_stderr(value: &impl Serialize) -> ExitCode {
    let Ok(mut encoded) = serde_json::to_vec(value) else {
        return ExitCode::FAILURE;
    };
    encoded.push(b'\n');
    let _ = std::io::stderr().lock().write_all(&encoded);
    ExitCode::FAILURE
}
