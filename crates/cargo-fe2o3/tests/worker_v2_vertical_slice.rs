#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const WORKER_ID: &str = "cargo-fe2o3-fixture-worker-v1";
static NEXT_TEST: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fe2o3-worker-v2-flow-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-worker-v2-fixture"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_config(
    directory: &TestDirectory,
    final_symbols: &[&str],
    selects_invocation: bool,
) -> PathBuf {
    let worker_bytes = fs::read(fixture()).unwrap();
    let selected_source = if selects_invocation {
        directory.0.join("workflow_fixture.rs")
    } else {
        directory.0.join("different_device_unit.rs")
    };
    let value = json!({
        "candidate_output_max_bytes": 4096,
        "final_symbols": final_symbols,
        "format": "fe2o3-worker-v2-config-v1",
        "limits": {
            "stderr_bytes": 1024,
            "stdout_bytes": 16384,
            "timeout_ms": 2000
        },
        "link_options": [
            {"name": "code-object-version", "value": "6"},
            {"name": "opt-level", "value": "2"},
            {"name": "strip-debug", "value": "true"},
            {"name": "verify-each", "value": "true"}
        ],
        "providers": [],
        "units": [{
            "crate_name": "workflow_fixture",
            "source": selected_source,
            "working_directory": directory.0
        }],
        "worker": {
            "byte_len": worker_bytes.len(),
            "llvm_build_identity": "cargo-fe2o3-fixture-llvm-v1",
            "path": fixture(),
            "sha256": hex(&Sha256::digest(&worker_bytes)),
            "worker_build_identity": WORKER_ID
        }
    });
    let path = directory.0.join("worker-v2.json");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

fn run_wrapper(directory: &TestDirectory, config: Option<&Path>, rustc_mode: &str) -> Output {
    let source = directory.0.join("workflow_fixture.rs");
    fs::write(&source, "fn main() {}\n").unwrap();
    let artifact_dir = directory.0.join("artifacts");
    fs::create_dir_all(&artifact_dir).unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .env_clear()
        .current_dir(&directory.0)
        .env("FE2O3_BINDING_WRAPPER_MODE_V1", "1")
        .env("FE2O3_BUILD_SESSION_V1", "11".repeat(16))
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_FIXTURE_RUSTC_MARKER", directory.0.join("spawned"))
        .env("FE2O3_FIXTURE_RUSTC_MODE", rustc_mode)
        .env("FE2O3_FIXTURE_SOURCE", &source)
        .env("FE2O3_HSACO_DIR", artifact_dir)
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .arg(fixture())
        .args(["--crate-name", "workflow_fixture"])
        .arg(&source)
        .arg("-Cmetadata=worker-v2-test");
    if let Some(config) = config {
        command.env("FE2O3_WORKER_V2_CONFIG_V1", config);
    }
    command.output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn consumes_and_executes_before_the_unimplemented_publication_boundary() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, &["workflow_kernel"], true);
    let output = run_wrapper(&directory, Some(&config), "publish");

    assert!(!output.status.success());
    assert!(directory.0.join("spawned").exists());
    let stderr = stderr(&output);
    assert!(
        stderr.contains("Worker V2 produced inert evidence identity sha256:"),
        "{stderr}"
    );
    let identity = stderr
        .split("inert evidence identity sha256:")
        .nth(1)
        .and_then(|suffix| suffix.get(..64))
        .expect("publication-boundary diagnostic carries an evidence identity");
    assert!(identity.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(identity, "00".repeat(32));
    assert!(stderr.contains("no authenticated publication adapter"));
    assert!(!stderr.contains("without an authorized device backend"));
}

#[test]
fn missing_handoff_fails_and_makes_the_attempt_terminal() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, &["workflow_kernel"], true);
    let first = run_wrapper(&directory, Some(&config), "no-handoff");
    assert!(!first.status.success());
    assert!(
        stderr(&first).contains("compiler-module handoff consumption failed"),
        "{}",
        stderr(&first)
    );

    fs::remove_file(directory.0.join("spawned")).unwrap();
    let retry = run_wrapper(&directory, Some(&config), "publish");
    assert!(!retry.status.success());
    assert!(
        !directory.0.join("spawned").exists(),
        "a failed exact attempt must not respawn rustc"
    );
}

#[test]
fn worker_mismatch_invalidates_the_attempt_before_completion() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, &["workflow_kernel", "workflow_mismatch"], true);
    let output = run_wrapper(&directory, Some(&config), "publish");

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("Worker V2 output bytes differ"), "{stderr}");
    assert!(!stderr.contains("invalidation also failed"), "{stderr}");
}

#[test]
fn missing_or_mismeasured_configuration_prevents_rustc_spawn() {
    let missing_directory = TestDirectory::new();
    let missing = run_wrapper(&missing_directory, None, "publish");
    assert!(!missing.status.success());
    assert!(!missing_directory.0.join("spawned").exists());
    assert!(
        stderr(&missing).contains("requires FE2O3_WORKER_V2_CONFIG_V1"),
        "{}",
        stderr(&missing)
    );

    let mismatched_directory = TestDirectory::new();
    let config = write_config(&mismatched_directory, &["workflow_kernel"], true);
    let mut value: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["worker"]["sha256"] = Value::String("00".repeat(32));
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    let mismatched = run_wrapper(&mismatched_directory, Some(&config), "publish");
    assert!(!mismatched.status.success());
    assert!(!mismatched_directory.0.join("spawned").exists());
    assert!(
        stderr(&mismatched).contains("Worker V2 setup failed"),
        "{}",
        stderr(&mismatched)
    );
}

#[test]
fn unselected_host_units_run_without_attempts_but_device_production_still_fails_closed() {
    let host_directory = TestDirectory::new();
    let host_config = write_config(&host_directory, &["workflow_kernel"], false);
    let host = run_wrapper(&host_directory, Some(&host_config), "no-handoff");
    assert!(host.status.success(), "{}", stderr(&host));
    assert_eq!(
        fs::read_to_string(host_directory.0.join("spawned")).unwrap(),
        "no-attempt"
    );

    let device_directory = TestDirectory::new();
    let device_config = write_config(&device_directory, &["workflow_kernel"], false);
    let device = run_wrapper(
        &device_directory,
        Some(&device_config),
        "device-requires-attempt",
    );
    assert_eq!(device.status.code(), Some(42));
    assert!(
        stderr(&device).contains("rejected a missing managed attempt"),
        "{}",
        stderr(&device)
    );
}
