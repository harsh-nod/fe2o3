use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let Some(report) = args.first().map(PathBuf::from) else {
        eprintln!("runner fixture requires a report path");
        return ExitCode::FAILURE;
    };
    let leaked_environment = env::vars_os()
        .filter_map(|(name, _)| {
            is_build_control(&name).then(|| name.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    let payload = args
        .get(1)
        .map(|value| hex(os_bytes(value)))
        .unwrap_or_default();
    let record = serde_json::json!({
        "artifact_fd_open": fs::symlink_metadata("/proc/self/fd/197").is_ok(),
        "backend_fd_open": fs::symlink_metadata("/proc/self/fd/198").is_ok(),
        "leaked_environment": leaked_environment,
        "payload_hex": payload,
        "preserved_environment_hex": env::var_os("RUNNER_CHAIN_ENV")
            .map(|value| hex(os_bytes(&value))),
    });
    if let Err(error) = fs::write(report, serde_json::to_vec(&record).expect("encode report")) {
        eprintln!("runner fixture could not write report: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
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
