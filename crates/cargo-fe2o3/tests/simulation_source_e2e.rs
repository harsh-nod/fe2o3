#![cfg(all(target_os = "linux", not(feature = "legacy-hsa-runtime")))]

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

#[test]
fn source_fixture_leaves_the_namespace_to_the_managed_wrapper() {
    let source = fs::read_to_string(
        workspace().join("crates/cargo-fe2o3/tests/fixtures/simulation-source-fill/src/lib.rs"),
    )
    .expect("read simulation source fixture");
    let syntax = syn::parse_file(&source).expect("parse simulation source fixture");
    let kernel_attributes = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(&function.attrs),
            _ => None,
        })
        .flatten()
        .filter(|attribute| attribute.path().is_ident("kernel"))
        .collect::<Vec<_>>();
    assert_eq!(
        kernel_attributes.len(),
        1,
        "simulation source fixture must contain exactly one kernel"
    );
    let arguments = kernel_attributes[0]
        .meta
        .require_list()
        .expect("kernel attribute arguments");
    let top_level_arguments = arguments
        .tokens
        .clone()
        .into_iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    assert!(
        top_level_arguments
            .iter()
            .any(|argument| argument == "typed"),
        "simulation source fixture must retain the typed kernel contract"
    );
    assert!(
        !top_level_arguments
            .iter()
            .any(|argument| argument == "namespace"),
        "simulation source fixture must use the managed wrapper-derived namespace"
    );
}

fn run_simulation(
    workspace: &Path,
    backend: &Path,
    request: &Path,
    result: &Path,
    target: &Path,
    path: &std::ffi::OsStr,
) -> std::process::Output {
    let manifest =
        workspace.join("crates/cargo-fe2o3/tests/fixtures/simulation-source-fill/Cargo.toml");
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
        .env_remove("CARGO_PROFILE_DEV_DEBUG")
        .output()
        .expect("run source simulation")
}

fn assert_hostile_simulation_value_does_not_suppress_host_contract(manifest: &Path, target: &Path) {
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--locked", "--manifest-path"])
        .arg(manifest)
        .arg("--target-dir")
        .arg(target)
        .env("FE2O3_SIMULATION_MODE_V1", "attacker")
        .env_remove("FE2O3_CRATE_BINDING_ID_V1")
        .output()
        .expect("check typed fixture with hostile simulation value");
    assert!(
        !output.status.success(),
        "hostile simulation value was accepted"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not resolve the fe2o3-host crate"),
        "hostile simulation value did not retain normal host resolution:\n{stderr}"
    );
}

fn assert_exact_fill_result(bytes: &[u8]) {
    let result: serde_json::Value = serde_json::from_slice(bytes).expect("JSON result");
    assert_eq!(result["schema"], "fe2o3-simulation-result-v1");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["authority"], "observation_only");
    assert_eq!(result["simulated"], true);
    assert_eq!(result["hardware_observed"], false);
    assert_eq!(result["hardware_validation"], false);
    assert_eq!(result["performance_prediction"], false);
    assert_eq!(
        result["target_profile"]["identity"],
        "amdgpu_64_little_endian_v1"
    );
    assert_eq!(result["target_profile"]["index_bits"], 64);
    assert_eq!(result["target_profile"]["max_workgroup_invocations"], 1024);
    assert_eq!(
        result["schedule"]["identity"],
        "workgroup_major_local_zyx_cooperative_v1"
    );
    let kir_sha256 = result["kir"]["sha256"].as_str().unwrap();
    assert_eq!(kir_sha256.len(), 64);
    assert!(kir_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(result["kir"]["canonical_bytes"].as_u64().unwrap() > 0);
    assert_eq!(result["counts"]["arguments"], 1);
    assert_eq!(result["counts"]["shared_buffers"], 0);
    assert_eq!(result["counts"]["invocations_executed"], 4);
    assert_eq!(result["counts"]["workgroups_visited"], 1);
    assert_eq!(result["counts"]["scheduled_slots_visited"], 64);
    assert_eq!(result["counts"]["steps_executed"], 48);
    assert_eq!(result["counts"]["events_emitted"], 0);
    assert_eq!(
        result["conflict_assessment"]["status"],
        "no_conflicts_observed"
    );
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
    let manifest =
        workspace.join("crates/cargo-fe2o3/tests/fixtures/simulation-source-fill/Cargo.toml");
    assert_hostile_simulation_value_does_not_suppress_host_contract(
        &manifest,
        &directory.0.join("target-hostile"),
    );
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
    let target = directory.0.join("target-shared");

    let first_result = directory.0.join("first.json");
    let first = run_simulation(
        &workspace,
        &backend,
        &request,
        &first_result,
        &target,
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
    assert!(
        !target.join("fe2o3").exists(),
        "successful simulation retained an fe2o3 generation"
    );

    let second_result = directory.0.join("second.json");
    let second = run_simulation(
        &workspace,
        &backend,
        &request,
        &second_result,
        &target,
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
    assert!(
        !target.join("fe2o3").exists(),
        "same-target simulation retained an fe2o3 generation"
    );

    let mut missing_document: serde_json::Value =
        serde_json::from_slice(&fs::read(&request).unwrap()).unwrap();
    missing_document["kernel"] = serde_json::Value::String("missing".to_owned());
    let missing_request = directory.0.join("missing-request.json");
    fs::write(
        &missing_request,
        serde_json::to_vec(&missing_document).unwrap(),
    )
    .unwrap();
    let missing_result = directory.0.join("missing-result.json");
    let missing = run_simulation(
        &workspace,
        &backend,
        &missing_request,
        &missing_result,
        &target,
        &path,
    );
    assert!(!missing.status.success(), "missing kernel unexpectedly ran");
    let missing_stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        missing_stderr.contains(
            r#""schema":"fe2o3-simulation-error-v1","status":"error","stage":"preflight","kind":"preflight_unknown_kernel""#
        ),
        "missing kernel did not emit the stable structured error:\n{missing_stderr}"
    );
    assert!(!missing_result.exists());
    assert!(
        !target.join("fe2o3").exists(),
        "failed request retained an fe2o3 generation"
    );

    let after_missing_result = directory.0.join("after-missing.json");
    let after_missing = run_simulation(
        &workspace,
        &backend,
        &request,
        &after_missing_result,
        &target,
        &path,
    );
    assert!(
        after_missing.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&after_missing.stdout),
        String::from_utf8_lossy(&after_missing.stderr)
    );
    let after_missing_bytes = fs::read(&after_missing_result).unwrap();
    assert_exact_fill_result(&after_missing_bytes);
    assert_eq!(after_missing_bytes, first_bytes);
    assert!(!target.join("fe2o3").exists());

    let occupied = run_simulation(
        &workspace,
        &backend,
        &request,
        &first_result,
        &target,
        &path,
    );
    assert!(
        !occupied.status.success(),
        "preexisting output was unexpectedly replaced"
    );
    assert_eq!(fs::read(&first_result).unwrap(), first_bytes);
    assert!(
        !target.join("fe2o3").exists(),
        "failed output publication retained an fe2o3 generation"
    );

    let after_occupied_result = directory.0.join("after-occupied.json");
    let after_occupied = run_simulation(
        &workspace,
        &backend,
        &request,
        &after_occupied_result,
        &target,
        &path,
    );
    assert!(
        after_occupied.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&after_occupied.stdout),
        String::from_utf8_lossy(&after_occupied.stderr)
    );
    let after_occupied_bytes = fs::read(&after_occupied_result).unwrap();
    assert_exact_fill_result(&after_occupied_bytes);
    assert_eq!(after_occupied_bytes, first_bytes);
    assert!(!target.join("fe2o3").exists());
    assert!(!marker.exists(), "cargo fe2o3 simulate invoked rocminfo");
}
