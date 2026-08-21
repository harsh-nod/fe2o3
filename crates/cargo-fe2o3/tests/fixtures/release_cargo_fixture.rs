use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::thread;
use std::time::{Duration, Instant};

const REPORT: &str = ".fe2o3-protected-release-cargo-report-v1.json";
const READY: &str = ".fe2o3-protected-release-cargo-ready-v1";
const HOLD: &str = ".fe2o3-protected-release-cargo-hold-v1";
const CONTINUE: &str = ".fe2o3-protected-release-cargo-continue-v1";
const SURVIVED: &str = ".fe2o3-protected-release-cargo-survived-v1";

fn main() -> ExitCode {
    let args = env::args_os().collect::<Vec<_>>();
    match args.get(1).and_then(|value| value.to_str()) {
        Some("config") => config(&args),
        Some("metadata") => metadata(),
        Some("build") => build(&args),
        Some(command) => {
            eprintln!("protected release Cargo fixture rejects {command:?}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("protected release Cargo fixture has no command");
            ExitCode::FAILURE
        }
    }
}

fn config(args: &[OsString]) -> ExitCode {
    let key = args.last().and_then(|value| value.to_str()).unwrap_or("");
    eprintln!("error: config value `{key}` is not set");
    ExitCode::from(101)
}

fn metadata() -> ExitCode {
    let root = workspace_root();
    let package_id = format!("path+file://{}#external-standalone@0.1.0", root.display());
    let document = serde_json::json!({
        "packages": [{
            "checksum": null,
            "id": package_id,
            "links": null,
            "manifest_path": root.join("Cargo.toml"),
            "name": "external-standalone",
            "source": null,
            "targets": [{"kind": ["bin"]}],
            "version": "0.1.0",
        }],
        "resolve": {
            "nodes": [{"dependencies": [], "id": package_id}],
            "root": package_id,
        },
        "target_directory": root.join("target"),
        "version": 1,
        "workspace_default_members": [package_id],
        "workspace_members": [package_id],
        "workspace_root": root,
    });
    println!("{document}");
    ExitCode::SUCCESS
}

fn build(args: &[OsString]) -> ExitCode {
    let root = workspace_root();
    let state = root.join("target");
    if let Err(error) = fs::create_dir_all(&state) {
        eprintln!("protected release Cargo fixture cannot create state directory: {error}");
        return ExitCode::FAILURE;
    }
    let mut parent_death_signal = 0;
    // SAFETY: PR_GET_PDEATHSIG writes one signal number to the supplied live pointer.
    if unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut parent_death_signal, 0, 0, 0) } != 0 {
        eprintln!(
            "protected release Cargo fixture cannot inspect parent-death signal: {}",
            std::io::Error::last_os_error()
        );
        return ExitCode::FAILURE;
    }
    let report = serde_json::json!({
        "pid": std::process::id(),
        "parent_pid": unsafe { libc::getppid() },
        "parent_death_signal": parent_death_signal,
        "args": args.iter().map(|value| value.to_string_lossy()).collect::<Vec<_>>(),
        "target": env::var_os("FE2O3_TARGET").map(|value| value.to_string_lossy().into_owned()),
        "wrapper": env::var_os("RUSTC_WORKSPACE_WRAPPER")
            .map(|value| value.to_string_lossy().into_owned()),
        "trampoline_path_input": env::var_os("FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1")
            .map(|value| value.to_string_lossy().into_owned()),
        "trampoline_digest_input": env::var_os("FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1")
            .map(|value| value.to_string_lossy().into_owned()),
        "managed_rustc_args": env::var_os("FE2O3_MANAGED_RUSTC_ARGS_V1")
            .map(|value| value.to_string_lossy().into_owned()),
    });
    if let Err(error) = fs::write(state.join(REPORT), report.to_string()) {
        eprintln!("protected release Cargo fixture cannot write report: {error}");
        return ExitCode::FAILURE;
    }

    if state.join(HOLD).is_file() {
        if let Err(error) = fs::write(state.join(READY), []) {
            eprintln!("protected release Cargo fixture cannot signal readiness: {error}");
            return ExitCode::FAILURE;
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while !state.join(CONTINUE).is_file() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        if let Err(error) = fs::write(state.join(SURVIVED), []) {
            eprintln!("protected release Cargo fixture cannot record survival: {error}");
            return ExitCode::FAILURE;
        }
    }

    let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
    let rustc = required_path("RUSTC");
    let status = Command::new(wrapper)
        .arg(rustc)
        .args([
            "--crate-name",
            "external_standalone",
            "-Cmetadata=protected-release-build",
        ])
        .arg(root.join("src/main.rs"))
        .env("CARGO_MANIFEST_DIR", &root)
        .status();
    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("protected release compile wrapper failed with {status}");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("protected release Cargo fixture cannot invoke wrapper: {error}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    let mut directory = env::current_dir().expect("resolve protected release Cargo cwd");
    loop {
        if directory.join("Cargo.toml").is_file() {
            return directory;
        }
        assert!(directory.pop(), "protected release Cargo has no workspace");
    }
}

fn required_path(variable: &str) -> PathBuf {
    Path::new(&env::var_os(variable).unwrap_or_else(|| panic!("missing required {variable}")))
        .to_path_buf()
}
