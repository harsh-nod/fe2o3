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
