use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args = env::args_os().collect::<Vec<_>>();
    if args.get(1).is_none_or(|argument| argument != "config")
        && let Err(error) = record_invocation(&args)
    {
        eprintln!("fake Cargo could not record invocation: {error}");
        return ExitCode::FAILURE;
    }

    match args.get(1).and_then(|argument| argument.to_str()) {
        Some("metadata") => metadata(),
        Some("config") => config_get(&args),
        Some("build") if is_backend_build(&args) => backend_build(&args),
        Some("build" | "run") => build_or_run(&args),
        Some(other) => {
            eprintln!("fake Cargo received unexpected subcommand {other:?}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("fake Cargo received no subcommand");
            ExitCode::FAILURE
        }
    }
}

fn config_get(args: &[OsString]) -> ExitCode {
    let key = args
        .last()
        .and_then(|argument| argument.to_str())
        .unwrap_or("");
    let value = if key == "build.target" {
        env::var_os("FE2O3_TEST_BUILD_TARGET_JSON")
    } else if key == "build.rustc-wrapper" {
        env::var_os("FE2O3_TEST_RUSTC_WRAPPER_JSON")
    } else if key == "build.rustc-workspace-wrapper" {
        env::var_os("FE2O3_TEST_RUSTC_WORKSPACE_WRAPPER_JSON")
    } else if key == "build" {
        env::var_os("FE2O3_TEST_BUILD_CONFIG_JSON")
    } else if key.starts_with("target.") && key.ends_with(".runner") {
        env::var_os("FE2O3_TEST_CONFIG_RUNNER_JSON")
    } else if key == "target" {
        env::var_os("FE2O3_TEST_TARGET_TABLE_JSON")
    } else if key == "profile" {
        env::var_os("FE2O3_TEST_PROFILE_CONFIG_JSON")
    } else {
        None
    };
    if let Some(value) = value {
        println!("{}", value.to_string_lossy());
        ExitCode::SUCCESS
    } else {
        eprintln!("error: config value `{key}` is not set");
        ExitCode::from(101)
    }
}

fn is_backend_build(args: &[OsString]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "-p" && pair[1] == "rustc-codegen-fe2o3")
}

fn backend_build(args: &[OsString]) -> ExitCode {
    let Some(index) = args.iter().position(|argument| argument == "--target-dir") else {
        eprintln!("fake backend build has no isolated target directory");
        return ExitCode::FAILURE;
    };
    let Some(target) = args.get(index + 1).map(PathBuf::from) else {
        eprintln!("fake backend build target directory has no value");
        return ExitCode::FAILURE;
    };
    let output = target.join("debug/librustc_codegen_fe2o3.so");
    if let Err(error) = fs::create_dir_all(output.parent().expect("backend output parent"))
        .and_then(|()| fs::write(&output, b"isolated built backend"))
    {
        eprintln!("fake backend build could not create output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn metadata() -> ExitCode {
    let workspace_root = required_path("FE2O3_TEST_WORKSPACE_ROOT");
    let target_directory = required_path("FE2O3_TEST_TARGET_DIRECTORY");
    let record = serde_json::json!({
        "packages": [],
        "target_directory": target_directory,
        "version": 1,
        "workspace_members": [],
        "workspace_root": workspace_root,
    });
    println!("{record}");
    ExitCode::SUCCESS
}

fn build_or_run(args: &[OsString]) -> ExitCode {
    #[cfg(target_os = "linux")]
    for descriptor in [197, 198] {
        if fs::symlink_metadata(format!("/proc/self/fd/{descriptor}")).is_ok() {
            eprintln!("fake Cargo inherited build capability fd {descriptor}");
            return ExitCode::FAILURE;
        }
    }
    if let Some(mode) = env::var_os("FE2O3_TEST_BUILD_SCRIPT_MODE") {
        let fixture = required_path("FE2O3_TEST_BUILD_SCRIPT_FIXTURE");
        let status = match Command::new(fixture).arg(mode).status() {
            Ok(status) => status,
            Err(error) => {
                eprintln!("fake Cargo could not launch build-script probe: {error}");
                return ExitCode::FAILURE;
            }
        };
        let report = required_path("FE2O3_TEST_BUILD_SCRIPT_REPORT");
        let report = fs::read_to_string(report).unwrap_or_default();
        let expected = match env::var("FE2O3_TEST_BUILD_SCRIPT_MODE").as_deref() {
            Ok("ordinary") => {
                status.success()
                    && report.contains("mode=ordinary")
                    && report.contains("backend_open=false")
                    && report.contains("artifact_open=false")
            }
            Ok("exec-wrapper") => {
                report.contains("mode=exec-wrapper")
                    && report.contains("backend_open=true")
                    && report.contains("artifact_open=true")
            }
            _ => false,
        };
        if !expected {
            eprintln!(
                "fake Cargo build-script probe produced unexpected status {status} and report {report:?}"
            );
            return ExitCode::FAILURE;
        }
    }
    let output = required_path("FE2O3_TEST_TARGET_DIRECTORY").join("fe2o3");
    if let Some(source) = env::var_os("FE2O3_TEST_REPLACE_BACKEND")
        && let Err(error) = fs::write(source, b"replacement backend")
    {
        eprintln!("fake Cargo could not replace backend source: {error}");
        return ExitCode::FAILURE;
    }
    let active = match env::var_os("FE2O3_TEST_EXCLUSIVE_ACTIVE") {
        Some(path) => {
            let path = PathBuf::from(path);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => Some(path),
                Err(error) => {
                    eprintln!("fake Cargo observed concurrent generation: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        None => None,
    };
    if let Some(milliseconds) = env::var("FE2O3_TEST_SLEEP_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(milliseconds));
    }
    if let Some(display) = env::var_os("FE2O3_TEST_SUBSTITUTE_ARTIFACT") {
        let display = PathBuf::from(display);
        let relocated = display.with_extension("relocated");
        let outside = display.with_extension("outside");
        if let Err(error) = substitute_artifact_path(&display, &relocated, &outside) {
            eprintln!("fake Cargo could not substitute artifact path: {error}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    let sidecar = if let Some(counter) = env::var_os("FE2O3_TEST_MUTATING_COUNTER") {
        let counter = PathBuf::from(counter);
        let next = fs::read_to_string(&counter)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
            + 1;
        if let Err(error) = fs::write(&counter, next.to_string()) {
            eprintln!("fake Cargo could not update mutation counter: {error}");
            return ExitCode::FAILURE;
        }
        format!("fixture-sidecar-{next}").into_bytes()
    } else {
        b"fixture-sidecar".to_vec()
    };
    if let Err(error) = fs::write(output.join("fixture.hsaco"), sidecar) {
        eprintln!("fake Cargo could not write sidecar: {error}");
        return ExitCode::FAILURE;
    }
    if let Some(marker) = env::var_os("FE2O3_TEST_FAIL_ONCE") {
        let marker = PathBuf::from(marker);
        if !marker.exists() {
            if let Err(error) = fs::write(&marker, b"failed") {
                eprintln!("fake Cargo could not write failure marker: {error}");
            }
            return ExitCode::from(23);
        }
    }
    if let Some(active) = active
        && let Err(error) = fs::remove_file(active)
    {
        eprintln!("fake Cargo could not clear active marker: {error}");
        return ExitCode::FAILURE;
    }
    if args.get(1).is_some_and(|argument| argument == "run")
        && let Err(error) = run_application_fixture(args)
    {
        eprintln!("fake Cargo could not execute injected runner: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_application_fixture(args: &[OsString]) -> Result<(), String> {
    let Some(application) = env::var_os("FE2O3_TEST_RUN_APPLICATION") else {
        return Ok(());
    };
    let report = required_path("FE2O3_TEST_RUN_APPLICATION_REPORT");
    let payload = env::var_os("FE2O3_TEST_RUN_APPLICATION_PAYLOAD").unwrap_or_default();
    let runner = args
        .windows(2)
        .filter(|pair| pair[0] == "--config")
        .filter_map(|pair| pair[1].to_str())
        .filter_map(|config| config.split_once(".runner=").map(|(_, value)| value))
        .next_back()
        .ok_or_else(|| "run invocation has no injected target runner".to_string())?;
    let runner: Vec<String> = serde_json::from_str(runner)
        .map_err(|error| format!("decode injected runner configuration: {error}"))?;
    let (program, prefix) = runner
        .split_first()
        .ok_or_else(|| "injected runner configuration is empty".to_string())?;
    let status = Command::new(program)
        .args(prefix)
        .arg(application)
        .arg(report)
        .arg(payload)
        .status()
        .map_err(|error| format!("launch injected runner: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("injected runner failed with status {status}"))
    }
}

#[cfg(unix)]
fn substitute_artifact_path(
    display: &Path,
    relocated: &Path,
    outside: &Path,
) -> Result<(), String> {
    use std::os::unix::fs::symlink;

    fs::rename(display, relocated)
        .map_err(|error| format!("rename artifact directory: {error}"))?;
    fs::create_dir(outside).map_err(|error| format!("create outside directory: {error}"))?;
    fs::write(outside.join("keep"), b"outside")
        .map_err(|error| format!("write outside sentinel: {error}"))?;
    symlink(outside, display).map_err(|error| format!("install replacement symlink: {error}"))
}

#[cfg(not(unix))]
fn substitute_artifact_path(
    _display: &Path,
    _relocated: &Path,
    _outside: &Path,
) -> Result<(), String> {
    Err("artifact substitution fixture requires Unix".to_string())
}

fn required_path(variable: &str) -> PathBuf {
    PathBuf::from(env::var_os(variable).unwrap_or_else(|| panic!("missing {variable}")))
}

fn record_invocation(args: &[OsString]) -> Result<(), String> {
    let path = required_path("FE2O3_TEST_CARGO_LOG");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    let cwd = env::current_dir().map_err(|error| format!("resolve cwd: {error}"))?;
    write_field(&mut file, cwd.as_os_str())?;
    write_u64(&mut file, args.len() as u64)?;
    for argument in args {
        write_field(&mut file, argument)?;
    }
    for variable in [
        "CARGO_TARGET_DIR",
        "FE2O3_HSACO_DIR",
        "FE2O3_TARGET",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "FE2O3_MANAGED_RUSTC_ARGS_V1",
    ] {
        write_field(
            &mut file,
            env::var_os(variable)
                .as_deref()
                .unwrap_or_else(|| OsStr::new("")),
        )?;
    }
    Ok(())
}

fn write_field(file: &mut impl Write, value: &OsStr) -> Result<(), String> {
    let bytes = os_bytes(value);
    write_u64(file, bytes.len() as u64)?;
    file.write_all(bytes)
        .map_err(|error| format!("write field: {error}"))
}

fn write_u64(file: &mut impl Write, value: u64) -> Result<(), String> {
    file.write_all(&value.to_le_bytes())
        .map_err(|error| format!("write length: {error}"))
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value
        .to_str()
        .expect("the Cargo fixture requires UTF-8 arguments off Unix")
        .as_bytes()
}
