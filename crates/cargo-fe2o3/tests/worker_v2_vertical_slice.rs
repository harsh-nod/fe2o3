#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};

include!("../../fe2o3-hsaco-finalize/tests/fixtures/worker_v2_hsaco_test_support.rs");

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

fn worker_fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-worker-v2-fixture"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_config(directory: &TestDirectory, selects_invocation: bool) -> PathBuf {
    write_config_with_output(directory, selects_invocation, None)
}

fn write_config_with_output(
    directory: &TestDirectory,
    selects_invocation: bool,
    worker_output: Option<&[u8]>,
) -> PathBuf {
    let worker_bytes = fs::read(worker_fixture()).unwrap();
    let selected_source = if selects_invocation {
        directory.0.join("workflow_fixture.rs")
    } else {
        directory.0.join("different_device_unit.rs")
    };
    let providers = worker_output.map_or_else(Vec::new, |bytes| {
        let path = directory.0.join("worker-output.hsaco");
        fs::write(&path, bytes).unwrap();
        vec![json!({
            "byte_len": bytes.len(),
            "kind": "amdgpu-relocatable",
            "path": path,
            "sha256": hex(&Sha256::digest(bytes))
        })]
    });
    let value = json!({
        "candidate_output_max_bytes": 67108864,
        "format": "fe2o3-worker-v2-config-v2",
        "limits": {
            "stderr_bytes": 1024,
            "stdout_bytes": 16384,
            "timeout_ms": 2000
        },
        "link_options": [
            {"name": "code-object-version", "value": "5"},
            {"name": "opt-level", "value": "2"},
            {"name": "strip-debug", "value": "true"},
            {"name": "verify-each", "value": "true"}
        ],
        "providers": providers,
        "units": [{
            "crate_name": "workflow_fixture",
            "source": selected_source,
            "working_directory": directory.0
        }],
        "worker": {
            "byte_len": worker_bytes.len(),
            "llvm_build_identity": "cargo-fe2o3-fixture-llvm-v1",
            "path": worker_fixture(),
            "sha256": hex(&Sha256::digest(&worker_bytes)),
            "worker_build_identity": WORKER_ID
        }
    });
    let path = directory.0.join("worker-v2.json");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

fn run_wrapper(directory: &TestDirectory, config: Option<&Path>, rustc_mode: &str) -> Output {
    wrapper_command(directory, config, rustc_mode)
        .output()
        .unwrap()
}

fn wrapper_command(directory: &TestDirectory, config: Option<&Path>, rustc_mode: &str) -> Command {
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
        .arg(worker_fixture())
        .args(["--crate-name", "workflow_fixture"])
        .arg(&source)
        .arg("-Cmetadata=worker-v2-test");
    if let Some(config) = config {
        command.env("FE2O3_WORKER_V2_CONFIG_V2", config);
    }
    command
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stage_ready_restart(directory: &TestDirectory) -> (PathBuf, PathBuf) {
    let config = write_config(directory, true);
    let handoff_marker = directory.0.join("handoff-ready");
    let mut first = wrapper_command(directory, Some(&config), "stop-after-handoff");
    first.env("FE2O3_FIXTURE_HANDOFF_MARKER", &handoff_marker);
    let mut first = first.spawn().unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let status = fs::read_to_string(format!("/proc/{}/status", first.id())).unwrap_or_default();
        if handoff_marker.exists() && status.lines().any(|line| line.starts_with("State:\tT")) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let status = fs::read_to_string(format!("/proc/{}/status", first.id())).unwrap();
    assert!(status.lines().any(|line| line.starts_with("State:\tT")));
    first.kill().unwrap();
    first.wait().unwrap();

    let attempt = fs::read_to_string(directory.0.join("spawned")).unwrap();
    let source = directory.0.join("workflow_fixture.rs");
    let artifact_dir = directory.0.join("artifacts");
    let staged = Command::new(worker_fixture())
        .arg("--stage-restart")
        .arg(&artifact_dir)
        .arg(&source)
        .arg(attempt.trim())
        .output()
        .unwrap();
    assert!(staged.status.success(), "{}", stderr(&staged));
    fs::remove_file(directory.0.join("spawned")).unwrap();
    (config, artifact_dir)
}

#[test]
fn valid_worker_output_persists_before_publication_and_cleans_exact_restart_state() {
    let directory = TestDirectory::new();
    let built = fixture(FixtureOptions {
        target: "gfx942:xnack-",
        code_object_version: 3,
        entry: "workflow_kernel",
        descriptor: "workflow_kernel.kd",
        ..FixtureOptions::valid()
    });
    assert!(built.text_offset < built.bytes.len());
    let provider = built.bytes;
    let mut output = provider.clone();
    output[built.text_offset] ^= 1;
    let config = write_config_with_output(&directory, true, Some(&provider));

    let result = run_wrapper(&directory, Some(&config), "publish-valid");
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(directory.0.join("spawned").exists());

    let artifact_dir = directory.0.join("artifacts");
    let entries = fs::read_dir(&artifact_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert!(entries.iter().any(|path| {
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".fe2o3-link-artifact-v1-")
            && fs::read(path).unwrap() == output
    }));
    assert!(entries.iter().all(|path| {
        let name = path.file_name().unwrap().to_string_lossy();
        !name.ends_with(".record")
            || (!name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                && !name.starts_with(".fe2o3-cargo-worker-v2-resume-v1-"))
    }));
}

#[test]
fn ready_intent_resumes_in_a_new_process_without_spawning_rustc() {
    let directory = TestDirectory::new();
    let (config, artifact_dir) = stage_ready_restart(&directory);
    let recovered = run_wrapper(&directory, Some(&config), "fail");
    assert!(recovered.status.success(), "{}", stderr(&recovered));
    assert!(
        !directory.0.join("spawned").exists(),
        "recovery unexpectedly spawned rustc"
    );
    let entries = fs::read_dir(&artifact_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert!(entries.iter().any(|path| {
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".fe2o3-link-artifact-v1-")
            && fs::read(path).unwrap() == b"restart-recovered-inert-worker-v2-output"
    }));
    assert!(entries.iter().all(|path| {
        let name = path.file_name().unwrap().to_string_lossy();
        !name.ends_with(".record")
            || (!name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                && !name.starts_with(".fe2o3-cargo-worker-v2-resume-v1-"))
    }));
}

#[test]
fn changed_worker_configuration_rejects_ready_intent_without_spawning_rustc() {
    let directory = TestDirectory::new();
    let (config, artifact_dir) = stage_ready_restart(&directory);
    let mut value: JsonValue = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["candidate_output_max_bytes"] = JsonValue::from(4096);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();

    let rejected = run_wrapper(&directory, Some(&config), "fail");
    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("different build session or invocation"),
        "{}",
        stderr(&rejected)
    );
    assert!(!directory.0.join("spawned").exists());
    assert!(fs::read_dir(&artifact_dir).unwrap().any(|entry| {
        let name = entry.unwrap().file_name();
        name.to_string_lossy()
            .starts_with(".fe2o3-cargo-worker-v2-resume-v1-")
            && name.to_string_lossy().ends_with(".record")
    }));
}

#[test]
fn invalid_worker_output_fails_independent_hsaco_inspection_without_publication() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, true);
    let output = run_wrapper(&directory, Some(&config), "publish");

    assert!(!output.status.success());
    assert!(directory.0.join("spawned").exists());
    let stderr = stderr(&output);
    assert!(
        stderr.contains("independent Worker V2 HSACO inspection failed")
            && stderr.contains("invalid ELF"),
        "{stderr}"
    );
    assert!(!stderr.contains("without an authorized device backend"));
    let artifact_dir = directory.0.join("artifacts");
    assert!(fs::read_dir(&artifact_dir).unwrap().all(|entry| {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy();
        !name.starts_with(".fe2o3-link-artifact-v1-")
            && !name.starts_with(".fe2o3-link-publication-v1-")
    }));

    fs::remove_file(directory.0.join("spawned")).unwrap();
    let retry = run_wrapper(&directory, Some(&config), "publish");
    assert!(!retry.status.success());
    assert!(
        !directory.0.join("spawned").exists(),
        "an admission-rejected attempt must remain terminal"
    );
}

#[test]
fn missing_handoff_fails_and_makes_the_attempt_terminal() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, true);
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
    let config = write_config(&directory, true);
    let output = run_wrapper(&directory, Some(&config), "publish-mismatch");

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
        stderr(&missing).contains("requires FE2O3_WORKER_V2_CONFIG_V2"),
        "{}",
        stderr(&missing)
    );

    let mismatched_directory = TestDirectory::new();
    let config = write_config(&mismatched_directory, true);
    let mut value: JsonValue = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["worker"]["sha256"] = JsonValue::String("00".repeat(32));
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
    let host_config = write_config(&host_directory, false);
    let host = run_wrapper(&host_directory, Some(&host_config), "no-handoff");
    assert!(host.status.success(), "{}", stderr(&host));
    assert_eq!(
        fs::read_to_string(host_directory.0.join("spawned")).unwrap(),
        "no-attempt"
    );

    let device_directory = TestDirectory::new();
    let device_config = write_config(&device_directory, false);
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
