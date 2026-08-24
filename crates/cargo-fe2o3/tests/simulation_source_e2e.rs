#![cfg(all(target_os = "linux", not(feature = "hardware-runtime")))]

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "cargo-fe2o3-simulation-source-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create simulation test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn run_simulation(
    workspace: &Path,
    backend: &Path,
    request: &Path,
    result: &Path,
    target: &Path,
    path: &std::ffi::OsStr,
) -> std::process::Output {
    let manifest = workspace
        .join("crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/Cargo.toml");
    Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .current_dir(workspace)
        .args(["simulate", "--request"])
        .arg(request)
        .arg("--output")
        .arg(result)
        .args(["--", "--locked", "--manifest-path"])
        .arg(manifest)
        .arg("--target-dir")
        .arg(target)
        .env("CARGO", env!("CARGO"))
        .env("FE2O3_BACKEND", backend)
        .env("PATH", path)
        .env_remove("FE2O3_TARGET")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .output()
        .expect("run source simulation")
}

fn assert_exact_fill_result(bytes: &[u8]) {
    let result: serde_json::Value = serde_json::from_slice(bytes).expect("JSON result");
    assert_eq!(result["schema"], "fe2o3-simulation-result-v1");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["authority"], "observation_only");
    assert_eq!(result["counts"]["invocations_executed"], 4);
    assert_eq!(result["counts"]["workgroups_visited"], 4);
    assert_eq!(result["counts"]["scheduled_slots_visited"], 4);
    assert_eq!(
        result["arguments"][0]["value"]["bytes"],
        "0x11000000110000001100000011000000"
    );
}

#[test]
#[ignore = "requires an explicitly built real rustc-codegen-fe2o3 backend"]
fn ordinary_source_simulation_is_exact_deterministic_and_never_probes_a_gpu() {
    let workspace = workspace();
    let backend = env::var_os("FE2O3_TEST_SIMULATION_BACKEND")
        .map(PathBuf::from)
        .expect("FE2O3_TEST_SIMULATION_BACKEND must name the real backend");
    assert!(backend.is_absolute() && backend.is_file());
    let directory = TestDirectory::new();
    let request = workspace.join("crates/cargo-fe2o3/tests/fixtures/simulate-fill-request-v1.json");
    let trap = directory.0.join("path");
    fs::create_dir(&trap).unwrap();
    let rocminfo = trap.join("rocminfo");
    let marker = directory.0.join("rocminfo-was-run");
    fs::write(
        &rocminfo,
        format!("#!/bin/sh\n: > '{}'\nexit 97\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&rocminfo, fs::Permissions::from_mode(0o700)).unwrap();
    let path = env::join_paths(
        std::iter::once(trap).chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap();

    let first_result = directory.0.join("first.json");
    let first = run_simulation(
        &workspace,
        &backend,
        &request,
        &first_result,
        &directory.0.join("target-first"),
        &path,
    );
    assert!(
        first.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stdout.is_empty());
    let first_bytes = fs::read(&first_result).unwrap();
    assert_exact_fill_result(&first_bytes);

    let second_result = directory.0.join("second.json");
    let second = run_simulation(
        &workspace,
        &backend,
        &request,
        &second_result,
        &directory.0.join("target-second"),
        &path,
    );
    assert!(
        second.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(second.stdout.is_empty());
    let second_bytes = fs::read(&second_result).unwrap();
    assert_exact_fill_result(&second_bytes);
    assert_eq!(second_bytes, first_bytes);
    assert!(!marker.exists(), "cargo fe2o3 simulate invoked rocminfo");
}
