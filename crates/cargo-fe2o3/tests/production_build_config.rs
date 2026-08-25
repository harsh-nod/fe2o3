#![cfg(not(feature = "qualification-oracles-test-only"))]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cargo-fe2o3-production-build-config-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create scratch directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn production_configuration_has_no_compatibility_type_alias() {
    let source = include_str!("../src/build_config.rs");
    assert!(!source.contains("type PreparedBuildConfig = PreparedProductionBuildConfig"));
    let production_api = source
        .split("impl PreparedProductionBuildConfig {")
        .nth(1)
        .expect("production configuration API exists")
        .split("fn prepare_production_manifest")
        .next()
        .expect("production parser follows its API");
    assert!(!production_api.contains("executes_worker_in_rustc"));
    assert!(!production_api.contains("into_production"));
}

#[test]
fn production_managed_transaction_has_no_qualification_dispatch() {
    let source = include_str!("../src/binding_wrapper.rs");
    let preparation = source
        .split("fn prepare_production_managed_attempt(")
        .nth(1)
        .expect("direct production preparation exists")
        .split("#[cfg(any(test, feature = \"qualification-oracles-test-only\"))]\nfn prepare_managed_attempt(")
        .next()
        .expect("qualification preparation follows production preparation");
    for rejected in [
        "PreparedManagedWork",
        "ManagedQualificationWork",
        "executes_worker_in_rustc",
        "WorkerV2ResumeStore",
        "row_softmax",
        "qualification",
    ] {
        assert!(
            !preparation.contains(rejected),
            "production preparation contains qualification decision {rejected}"
        );
    }
    assert!(preparation.contains("prepare_managed_production_build"));
    assert!(preparation.contains("production_build: Some(production_build)"));

    let completion = source
        .split("fn complete_managed_attempt_inner(")
        .nth(1)
        .expect("direct production completion exists")
        .split("#[cfg(any(test, feature = \"qualification-oracles-test-only\"))]\nfn complete_managed_attempt_inner(")
        .next()
        .expect("qualification completion follows production completion");
    assert!(!completion.contains("finish_build_attempt"));
    assert!(!completion.contains("qualification_work"));
    assert!(completion.contains("production_build.take().ok_or_else"));
    assert!(completion.contains("complete_managed_production_build"));

    let environment = source
        .split("fn materialize_production_child_environment(")
        .nth(1)
        .expect("direct production child environment exists")
        .split("#[cfg(any(test, feature = \"qualification-oracles-test-only\"))]\nfn materialize_reviewed_child_environment(")
        .next()
        .expect("qualification child environment follows production environment");
    assert!(!environment.contains("GeneralGemm"));
    assert!(!environment.contains("WorkerV2"));
    assert!(!environment.contains("qualification"));
}

#[test]
fn production_driver_rejects_qualification_before_target_preparation() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .args(["build", "--target", "host-placeholder"])
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FE2O3_QUALIFICATION_ORACLE_V1 is unavailable")
            && stderr.contains("production compilation has no selector"),
        "{stderr}"
    );
    assert!(!stderr.contains("production-v1 requires exact FE2O3_TARGET"));
}

#[test]
fn production_manifest_rejects_qualification_envelope_fields() {
    let scratch = ScratchDirectory::new();
    let manifest = scratch.0.join("build-config.json");
    fs::write(
        &manifest,
        br#"{"candidate_output_max_bytes":1,"format":"fe2o3-production-build-config-v1","limits":{},"link_options":[],"load_envelope":"required","load_envelope_inputs":{},"providers":[],"units":[],"worker":{}}"#,
    )
    .expect("write manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_PRODUCTION_BUILD_CONFIG_V1", &manifest)
        .args(["build", "--target", "amdgcn-amd-amdhsa"])
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("production configuration must contain exactly the fields")
            && stderr.contains("load_envelope"),
        "{stderr}"
    );
}

#[test]
fn production_rejects_worker_v2_namespace_before_reading_its_manifest() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .env(
            "FE2O3_WORKER_V2_CONFIG_V2",
            "/does/not/exist/worker-v2-config.json",
        )
        .args(["build", "--target", "amdgcn-amd-amdhsa"])
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FE2O3_WORKER_V2_CONFIG_V2 is qualification-only")
            && stderr.contains("FE2O3_PRODUCTION_BUILD_CONFIG_V1"),
        "{stderr}"
    );
    assert!(!stderr.contains("does/not/exist"), "{stderr}");
}
