use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("runner chain fixture: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let separator = args
        .iter()
        .position(|argument| argument == "--runner-end")
        .ok_or_else(|| "missing --runner-end".to_string())?;
    let prefix = &args[..separator];
    let application = args
        .get(separator + 1)
        .ok_or_else(|| "missing application".to_string())?;
    let application_args = &args[separator + 2..];
    let report = prefix
        .windows(2)
        .find(|pair| pair[0] == "--report")
        .map(|pair| PathBuf::from(&pair[1]))
        .ok_or_else(|| "missing --report path".to_string())?;
    let leaked_environment = env::vars_os()
        .filter_map(|(name, _)| is_build_control(&name).then(|| hex(os_bytes(&name))))
        .collect::<Vec<_>>();
    let record = serde_json::json!({
        "artifact_fd_open": fs::symlink_metadata("/proc/self/fd/197").is_ok(),
        "backend_fd_open": fs::symlink_metadata("/proc/self/fd/198").is_ok(),
        "prefix_hex": prefix.iter().map(|value| hex(os_bytes(value))).collect::<Vec<_>>(),
        "application_hex": hex(os_bytes(application)),
        "application_args_hex": application_args.iter().map(|value| hex(os_bytes(value))).collect::<Vec<_>>(),
        "preserved_environment_hex": env::var_os("RUNNER_CHAIN_ENV")
            .map(|value| hex(os_bytes(&value))),
        "leaked_environment": leaked_environment,
    });
    fs::write(
        report,
        serde_json::to_vec(&record).map_err(|error| format!("encode report: {error}"))?,
    )
    .map_err(|error| format!("write report: {error}"))?;

    let status = Command::new(application)
        .args(application_args)
        .status()
        .map_err(|error| format!("launch application: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("application failed with status {status}"))
    }
}

fn is_build_control(name: &OsStr) -> bool {
    let bytes = os_bytes(name);
    bytes.starts_with(b"FE2O3_")
        || matches!(
            bytes,
            b"RUSTFLAGS"
                | b"CARGO_ENCODED_RUSTFLAGS"
                | b"RUSTC_WRAPPER"
                | b"RUSTC_WORKSPACE_WRAPPER"
        )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value.to_str().expect("UTF-8 value off Unix").as_bytes()
}
