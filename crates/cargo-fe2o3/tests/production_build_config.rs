use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
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
fn unit_test_configuration_cannot_select_qualification_code() {
    for (name, source) in [
        ("main", include_str!("../src/main.rs")),
        (
            "build configuration",
            include_str!("../src/build_config.rs"),
        ),
        ("binding wrapper", include_str!("../src/binding_wrapper.rs")),
        (
            "capability broker",
            include_str!("../src/capability_broker.rs"),
        ),
    ] {
        assert!(
            !source.contains("any(test, feature = \"qualification-oracles-test-only\")"),
            "{name} still changes pipeline behavior under cfg(test)"
        );
    }
}

#[test]
fn cargo_and_application_routes_are_feature_invariant() {
    let source = include_str!("../src/main.rs");
    let cargo_route = source
        .split("fn cargo_with_backend_result(")
        .nth(1)
        .expect("Cargo production route exists")
        .split("fn authority_sha256_from_environment(")
        .next()
        .expect("Cargo production route has a bounded body");
    assert!(!cargo_route.contains("qualification-oracles-test-only"));
    assert!(cargo_route.contains("PreparedProductionBuildConfig"));
    assert!(cargo_route.contains("ProductionCargoPlan"));

    let application_route = source
        .split("fn run_application_boundary_result(")
        .nth(1)
        .expect("application production route exists")
        .split("fn run_application_with_handoff(")
        .next()
        .expect("application production route has a bounded body");
    assert!(!application_route.contains("qualification-oracles-test-only"));
    assert!(application_route.contains("RUNNER_EXPECTS_ENVELOPE"));
    assert!(application_route.contains("requires a canonical load envelope"));
}

#[test]
fn production_managed_transaction_has_no_qualification_dispatch() {
    let source = include_str!("../src/binding_wrapper.rs");
    let preparation = source
        .split("fn prepare_production_managed_attempt(")
        .nth(1)
        .expect("direct production preparation exists")
        .split("#[cfg(feature = \"qualification-oracles-test-only\")]\nfn prepare_managed_attempt(")
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
    assert!(preparation.contains("production_build,"));
    assert!(!preparation.contains("production_build: Option"));

    let completion = source
        .split("fn complete_managed_attempt_inner(")
        .nth(1)
        .expect("direct production completion exists")
        .split("#[cfg(feature = \"qualification-oracles-test-only\")]\nfn complete_managed_attempt_inner(")
        .next()
        .expect("qualification completion follows production completion");
    assert!(!completion.contains("finish_build_attempt"));
    assert!(!completion.contains("qualification_work"));
    assert!(completion.contains("managed.production_build"));
    assert!(!completion.contains("production_build.take()"));
    assert!(completion.contains("complete_managed_production_build"));

    let environment = source
        .split("fn materialize_production_child_environment(")
        .nth(1)
        .expect("direct production child environment exists")
        .split("#[cfg(feature = \"qualification-oracles-test-only\")]\nfn materialize_reviewed_child_environment(")
        .next()
        .expect("qualification child environment follows production environment");
    assert!(!environment.contains("GeneralGemm"));
    assert!(!environment.contains("WorkerV2"));
    assert!(!environment.contains("qualification"));
}

#[test]
fn production_capability_intake_releases_oracle_authority_immediately() {
    let source = include_str!("../src/binding_wrapper.rs");
    let broker = include_str!("../src/capability_broker.rs");
    let intake = source
        .split("fn from_production_environment(")
        .nth(1)
        .expect("direct production capability intake exists")
        .split("#[cfg(feature = \"qualification-oracles-test-only\")]\n    fn from_qualification_environment(")
        .next()
        .expect("qualification capability intake follows production intake");
    assert!(intake.contains(".invocation_authority"));
    assert!(intake.contains(".release()"));
    assert!(!intake.contains("ROW_SOFTMAX"));
    assert!(!intake.contains("FE2O3_QUALIFICATION"));
    assert!(!intake.contains("Some(invocation_authority)"));

    assert!(source.contains(
        "#[cfg(feature = \"qualification-oracles-test-only\")]\n    invocation_authority: Option<capability_broker::BrokeredInvocationAuthorityV1>"
    ));
    assert!(broker.contains(
        "Ordinary,\n        #[cfg(feature = \"qualification-oracles-test-only\")]\n        S09,"
    ));
    assert!(broker.contains(
        "#[cfg(feature = \"qualification-oracles-test-only\")]\n        pub(crate) fn inherit_for_child("
    ));
    assert!(broker.contains(
        "#[cfg(feature = \"qualification-oracles-test-only\")]\n        pinned_cargo_image: File,"
    ));
}

#[test]
fn production_run_has_one_worker_v3_application_path() {
    let source = include_str!("../src/main.rs");
    let injection = source
        .split("fn inject_production_application_runner(")
        .nth(1)
        .expect("production runner injection exists")
        .split("fn application_runner_executable(")
        .next()
        .expect("production runner injection has a bounded body");
    assert!(injection.contains("RUNNER_EXPECTS_ENVELOPE"));
    assert!(injection.contains("does not permit an intermediate Cargo runner"));
    assert!(!injection.contains("RUNNER_EXPECTS_NO_ENVELOPE"));
    assert!(!injection.contains("expects_envelope"));

    let execution = source
        .split("if runner_count != 0 || !original_runner.is_empty()")
        .nth(1)
        .expect("production runner execution exists")
        .split("fn run_application_with_handoff(")
        .next()
        .expect("production runner execution has a bounded body");
    assert!(execution.contains("requires a canonical load envelope"));
    assert!(execution.contains("run_application_with_handoff"));
    assert!(!execution.contains("RUNNER_EXPECTS_NO_ENVELOPE"));
    assert!(!execution.contains("run_qualification_application_without_handoff"));

    let handoff_source = include_str!("../src/application_handoff.rs");
    assert!(!handoff_source.contains("RUNNER_EXPECTS_NO_ENVELOPE"));
    let exec_source = include_str!("../src/application_exec.rs");
    assert!(!exec_source.contains("configure_closed_descriptor_baseline"));
}

#[test]
fn production_build_has_one_fixed_device_then_host_plan() {
    let source = include_str!("../src/main.rs");
    let plan = include_str!("../src/production_cargo_plan.rs");
    assert!(source.contains("ProductionCargoPlan::new"));
    assert!(source.contains("run_production_host_cargo"));
    assert!(source.contains("host phase uses ordinary rustc"));
    assert!(plan.contains("command: \"build\""));
    assert!(plan.contains("PRODUCTION_GFX942_RUSTC_TARGET_V1"));
    assert!(plan.contains("reject_caller_target"));
    assert!(!plan.contains("enum Pipeline"));
    assert!(!plan.contains("selector"));
}

#[test]
fn ordinary_host_phase_has_no_device_compiler_controls() {
    let source = include_str!("../src/main.rs");
    let host_phase = source
        .split("fn run_production_host_cargo(")
        .nth(1)
        .expect("host phase exists")
        .split("fn scrub_simulation_build_environment(")
        .next()
        .expect("host phase ends before environment scrubbing helpers");

    for removed in [
        ".env_remove(BACKEND_ENV)",
        ".env_remove(TARGET_ENV)",
        ".env_remove(build_config::PRODUCTION_BUILD_CONFIG_ENV)",
        ".env_remove(build_config::QUALIFICATION_ORACLE_ENV)",
        ".env_remove(capability_broker::CAPABILITY_BROKER_ENV)",
    ] {
        assert!(host_phase.contains(removed), "missing {removed}");
    }
    assert!(host_phase.contains("configure_pinned_rustc_child"));
    assert!(host_phase.contains("generation.reject_if_substituted"));
    assert!(!host_phase.contains("configure_production_target_environment"));
}

#[test]
fn production_runner_rejects_no_envelope_marker() {
    let scratch = ScratchDirectory::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700))
        .expect("make scratch generation private");
    let mut owner_record = b"fe2o3-owned-v1\0".to_vec();
    owner_record.extend_from_slice(&[1_u8; 16]);
    let owner_path = scratch.0.join(".fe2o3-owned-v1");
    fs::write(&owner_path, owner_record).expect("write generation owner record");
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600))
        .expect("make generation owner record private");
    let metadata = fs::metadata(&scratch.0).expect("stat scratch directory");
    let encoded_path = scratch
        .0
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .args([
            "__fe2o3-runner-v1".to_owned(),
            "3".to_owned(),
            encoded_path,
            metadata.dev().to_string(),
            metadata.ino().to_string(),
            "none".to_owned(),
            "0".to_owned(),
            "/bin/true".to_owned(),
        ])
        .output()
        .expect("run production application boundary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("production application runner requires the Worker V3 envelope marker"),
        "{stderr}"
    );
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
    assert!(!stderr.contains("production compilation requires exact FE2O3_TARGET"));
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
        .arg("build")
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
        .arg("build")
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

#[test]
fn production_driver_rejects_caller_target_selection() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .env_clear()
        .env("FE2O3_TARGET", "gfx942")
        .args(["build", "--target", "amdgcn-amd-amdhsa"])
        .output()
        .expect("run cargo-fe2o3");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("owns device and host target selection"),
        "{stderr}"
    );
}
