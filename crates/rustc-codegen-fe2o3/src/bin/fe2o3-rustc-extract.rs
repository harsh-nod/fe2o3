#![feature(rustc_private)]

use std::env;
use std::ffi::OsString;
use std::process::{Command, ExitStatus};

use fe2o3_rustc_invocation::{RustcInvocationV2, classify_rustc_invocation_v2};

const EXTRACT_CRATE_ENV_V1: &str = "FE2O3_EXTRACT_CRATE_V1";

fn main() {
    let code = match run(env::args_os().collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("fe2o3 rustc extraction: {error}");
            1
        }
    };
    std::process::exit(code);
}

fn run(argv: Vec<OsString>) -> Result<i32, String> {
    let actual_rustc_argv = argv
        .get(1..)
        .filter(|argv| !argv.is_empty())
        .ok_or_else(|| "wrapper requires the actual rustc argv".to_owned())?;
    let invocation = classify_rustc_invocation_v2(actual_rustc_argv)
        .map_err(|error| format!("invalid rustc invocation: {error}"))?;
    let selected_crate = match env::var_os(EXTRACT_CRATE_ENV_V1) {
        None => return passthrough(invocation),
        Some(value) => value
            .into_string()
            .map_err(|_| format!("{EXTRACT_CRATE_ENV_V1} must be valid UTF-8"))?,
    };
    if selected_crate.is_empty() {
        return Err(format!("{EXTRACT_CRATE_ENV_V1} must not be empty"));
    }

    let RustcInvocationV2::Compile(compile) = invocation else {
        return passthrough(invocation);
    };
    if compile.crate_name() != selected_crate {
        return passthrough(invocation);
    }

    let args = actual_rustc_argv
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "selected extraction argv must be valid UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    rustc_codegen_fe2o3::run_production_extraction_driver_v1(&args)?;
    Ok(0)
}

fn passthrough(invocation: RustcInvocationV2<'_>) -> Result<i32, String> {
    let status = Command::new(invocation.executable())
        .args(invocation.forwarded_args())
        .status()
        .map_err(|error| format!("failed to execute rustc passthrough: {error}"))?;
    Ok(exit_code(status))
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}
