//! Actual Rust -> checked KIR20 -> CPU/LLVM/normal inert descriptor handoff.
//! Each driver session retains actual rustc custody; files are diagnostic only.
//! Unresolved runtime ABI conditions are not discharged by these tests.
//! No native worker, protected finalizer or GPU execution is run.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::gfx942_inline_value_qualification_v30_tests::{
    checked, invocation_for_fixture, read_bounded, sanitized,
};
use super::{Callbacks, Compilation, Compiler, TyCtxt};

#[path = "gfx942_physical_entry_production_observation_v20_tests.rs"]
mod observation;

const OUTPUT_ENV: &str = "FE2O3_TEST_PHYSICAL_PRODUCTION_OUTPUT_V20";
const CHILD_ENV: &str = "FE2O3_TEST_PHYSICAL_PRODUCTION_INPUTS_V20";
const FEATURE_ENV: &str = "FE2O3_TEST_PHYSICAL_PRODUCTION_FEATURE_V20";
const PACKAGE: &str = "fe2o3-production-extraction-fixture";
const CRATE_NAME: &str = "fe2o3_production_extraction_fixture";
const CHILD: &str = "production_rustc_driver_v1::gfx942_physical_entry_production_v20_tests::actual_physical_entry_production_child";
const PREFIX: &str = "FE2O3_PHYSICAL_PRODUCTION_OBSERVATION_V20 ";
const MODE_ENV: &str = "FE2O3_TEST_PHYSICAL_PRODUCTION_MODE_V20";
const MODES: [&str; 3] = ["observe", "llvm", "handoff"];
const FEATURES: [&str; 8] = [
    "physical-entry-one-v20",
    "physical-entry-diamond-v20",
    "physical-entry-registers-v20",
    "physical-entry-wrong-launch-v20",
    "physical-entry-foreign-input-v20",
    "physical-entry-undefined-merge-v20",
    "physical-entry-missing-wait-v20",
    "physical-entry-wrong-carry-v20",
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/production-extraction-device")
        .canonicalize()
        .unwrap()
}

fn source_hash(path: &Path) -> String {
    super::lower_hex_v1(&Sha256::digest(
        read_bounded(path, 16 * 1024 * 1024).unwrap(),
    ))
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}

fn require_current_source() {
    for (relative, expected) in [
        (
            "src/lib.rs",
            include_bytes!("../../tests/fixtures/production-extraction-device/src/lib.rs")
                .as_slice(),
        ),
        (
            "src/physical_entry_v20.rs",
            include_bytes!(
                "../../tests/fixtures/production-extraction-device/src/physical_entry_v20.rs"
            )
            .as_slice(),
        ),
        (
            "Cargo.toml",
            include_bytes!("../../tests/fixtures/production-extraction-device/Cargo.toml")
                .as_slice(),
        ),
    ] {
        assert_eq!(
            read_bounded(&fixture().join(relative), 1024 * 1024).unwrap(),
            expected,
            "rebuild this harness after changing fixture {relative}"
        );
    }
}

fn checked_feature(feature: &str) -> Result<(), &'static str> {
    FEATURES
        .contains(&feature)
        .then_some(())
        .ok_or("unknown or combined physical-entry feature")
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PreparedInvocation {
    schema: String,
    feature: String,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
    source_sha256: String,
    root_source_sha256: String,
    manifest_sha256: String,
    artifacts_sha256: String,
    metadata_sha256: String,
}

fn derive_record(directory: &Path, feature: &str) -> PreparedInvocation {
    checked_feature(feature).unwrap();
    let fixture = fixture();
    let (args, crate_binding, cargo_observation) =
        invocation_for_fixture(directory, &fixture, PACKAGE, CRATE_NAME, Some(feature));
    PreparedInvocation {
        schema: "fe2o3-test-source-physical-production-invocation-v20".into(),
        feature: feature.into(),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: source_hash(&fixture.join("src/physical_entry_v20.rs")),
        root_source_sha256: source_hash(&fixture.join("src/lib.rs")),
        manifest_sha256: source_hash(&fixture.join("Cargo.toml")),
        artifacts_sha256: source_hash(&directory.join("dependencies.stdout")),
        metadata_sha256: source_hash(&directory.join("metadata.stdout")),
    }
}

fn digest(bytes: &[u8]) -> String {
    super::lower_hex_v1(&Sha256::digest(bytes))
}

fn rejection_fragment(feature: &str) -> Result<&'static str, &'static str> {
    match feature {
        "physical-entry-wrong-launch-v20" => {
            Ok("physical-entry requires exact authored launch64 and max_grid2")
        }
        "physical-entry-foreign-input-v20" => {
            Ok("physical-entry marker operands differ from exact root argument order")
        }
        "physical-entry-undefined-merge-v20" => {
            Ok("physical-entry reads an undefined physical register")
        }
        "physical-entry-missing-wait-v20" => Ok("physical load used before lgkm wait"),
        "physical-entry-wrong-carry-v20" => Ok("physical pointer high origin"),
        _ => Err("positive or unknown source must not be refused"),
    }
}

fn expected_rejection(feature: &str, diagnostic: &str) -> Result<(), &'static str> {
    checked_feature(feature)?;
    if diagnostic.len() > 64 * 1024 {
        return Err("diagnostic exceeds bound");
    }
    diagnostic
        .contains(rejection_fragment(feature)?)
        .then_some(())
        .ok_or("wrong source rejection boundary")
}

struct BodyCallbacks<'a> {
    feature: &'a str,
    output: &'a Path,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for BodyCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            let transaction = super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            if !transaction.has_authenticated_physical_entry_v20() {
                return Err("actual physical-entry source marker was not authenticated".into());
            }
            let target = transaction
                .lower_physical_entry_target_v20()
                .map_err(|error| error.to_string())?;
            observation::observe(target, self.feature, self.output)
        })());
        Compilation::Stop
    }
}

fn checked_mode(mode: &str) -> Result<(), &'static str> {
    MODES
        .contains(&mode)
        .then_some(())
        .ok_or("unknown extraction mode")
}

#[test]
#[ignore = "isolated actual AMD rustc child; use actual_physical_entry_production_ladder"]
fn actual_physical_entry_production_child() {
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("missing preparation directory"));
    let feature = std::env::var(FEATURE_ENV).expect("missing feature");
    let mode = std::env::var(MODE_ENV).expect("missing mode");
    checked_feature(&feature).unwrap();
    checked_mode(&mode).unwrap();
    require_current_source();
    let actual = derive_record(&directory, &feature);
    let retained: PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{feature}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, retained, "stale or substituted preparation");
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        actual.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        actual.cargo_observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let output = directory.join(format!("{feature}.{mode}"));
    let result = match mode.as_str() {
        "observe" => {
            let mut callbacks = BodyCallbacks {
                feature: &feature,
                output: &output,
                calls: 0,
                result: None,
            };
            rustc_driver::run_compiler(&actual.args, &mut callbacks);
            assert_eq!(callbacks.calls, 1);
            callbacks.result.expect("actual callback not reached")
        }
        "llvm" => super::run_production_gfx942_llvm_extraction_driver_v1(&actual.args, &output)
            .map(|()| {
                let bytes = read_bounded(&output, 64 * 1024).unwrap();
                json!({"stage":"public_normal_checked_llvm_driver","llvm_sha256":digest(&bytes)})
            }),
        "handoff" => super::run_production_gfx942_compiler_handoff_extraction_driver_v1(
            &actual.args,
            &output,
        )
        .map(|()| {
            let bytes = read_bounded(&output, 1024 * 1024).unwrap();
            let decoded = fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(&bytes).unwrap();
            assert!(
                !decoded.authenticates_compiler_origin() && !decoded.grants_compiler_authority()
            );
            json!({"stage":"public_normal_inert_handoff_driver","handoff_sha256":digest(&bytes)})
        }),
        _ => unreachable!(),
    };
    let observation = if FEATURES[..3].contains(&feature.as_str()) {
        result.unwrap()
    } else {
        let diagnostic = result.expect_err("invalid source unexpectedly admitted");
        expected_rejection(&feature, &diagnostic).unwrap();
        assert!(!output.exists());
        json!({"stage": "exact_source_profile_refused", "diagnostic": diagnostic})
    };
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    println!(
        "\n{PREFIX}{}",
        serde_json::to_string(&json!({
            "schema": "fe2o3-test-source-physical-production-observation-v20",
            "feature": feature, "mode": mode, "invocation": actual, "observation": observation,
            "actual_rustc_callback": true, "source_unchanged": true,
            "protected_finalizer_admitted": false, "native_llvm_executed": false,
            "grants_artifact_or_launch_authority": false, "hardware_observed": false,
        }))
        .unwrap()
    );
}

#[test]
#[ignore = "pinned-nightly real-source ladder; serialize Cargo and provide a fresh absolute output directory"]
fn actual_physical_entry_production_ladder() {
    let directory = PathBuf::from(
        std::env::var_os(OUTPUT_ENV).expect("set a fresh task-owned output directory"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    require_current_source();
    let rustc_path = PathBuf::from(
        std::env::var_os("RUSTC")
            .expect("set RUSTC to absolute pinned-nightly compiler; no manager auto-install"),
    );
    assert!(rustc_path.is_absolute());
    let mut rustc = Command::new(rustc_path);
    let sysroot = checked(
        sanitized(&mut rustc).args(["--print", "sysroot"]),
        &directory,
        "sysroot",
        None,
    );
    let sysroot = PathBuf::from(std::str::from_utf8(&sysroot).unwrap().trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    let mut metadata = Command::new(sysroot.join("bin/cargo"));
    checked(
        sanitized(&mut metadata)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(fixture().join("Cargo.toml")),
        &directory,
        "metadata",
        None,
    );
    let target = directory.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo).current_dir(repository()).args([
        "check", "--release", "--locked", "--offline", "-Zbuild-std=core", "-p", "fe2o3-device",
        "--target", "amdgcn-amd-amdhsa", "--message-format=json", "--manifest-path",
    ]).arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&target)
      .env("RUSTC", sysroot.join("bin/rustc"))
      .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
           "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
      &directory, "dependencies", Some(&target));
    fs::create_dir(directory.join("analysis-output")).unwrap();
    let mut observations = Vec::new();
    for feature in FEATURES {
        let record = derive_record(&directory, feature);
        fs::write(
            directory.join(format!("{feature}.invocation.json")),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        for mode in MODES {
            let mut child = Command::new(std::env::current_exe().unwrap());
            let stdout = checked(
                sanitized(&mut child)
                    .current_dir(repository())
                    .args(["--exact", CHILD, "--ignored", "--nocapture"])
                    .env(CHILD_ENV, &directory)
                    .env(FEATURE_ENV, feature)
                    .env(MODE_ENV, mode)
                    .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
                    .env(
                        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                        &record.cargo_observation,
                    )
                    .env("CARGO_MANIFEST_DIR", fixture())
                    .env("CARGO_PKG_NAME", PACKAGE)
                    .env("CARGO_PKG_VERSION", "0.1.0")
                    .env("CARGO_CRATE_NAME", CRATE_NAME),
                &directory,
                &format!("{feature}.{mode}"),
                None,
            );
            let stdout = std::str::from_utf8(&stdout).unwrap();
            let lines = stdout
                .lines()
                .filter_map(|line| line.strip_prefix(PREFIX))
                .collect::<Vec<_>>();
            assert_eq!(lines.len(), 1);
            assert_eq!(
                stdout
                    .lines()
                    .filter(
                        |line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;")
                    )
                    .count(),
                1
            );
            let observation: Value = serde_json::from_str(lines[0]).unwrap();
            assert_eq!(observation["feature"], feature);
            assert_eq!(observation["mode"], mode);
            assert_eq!(
                observation["invocation"],
                serde_json::to_value(&record).unwrap()
            );
            observations.push(observation);
        }
        if FEATURES[..3].contains(&feature) {
            for (mode, file, cap) in [
                ("llvm", "canonical.ll", 64 * 1024),
                ("handoff", "handoff-v2.bin", 1024 * 1024),
            ] {
                let observed = read_bounded(
                    &directory.join(format!("{feature}.observe")).join(file),
                    cap,
                )
                .unwrap();
                let public =
                    read_bounded(&directory.join(format!("{feature}.{mode}")), cap).unwrap();
                assert_eq!(
                    observed, public,
                    "normal driver differs from the same current checked source owner"
                );
            }
        }
    }
    require_current_source();
    assert_eq!(observations.len(), 24);
    fs::write(
        directory.join("observation.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "fe2o3-test-source-physical-production-ladder-v20", "observations": observations,
            "public_driver_output_relation": "exact_bytes_same_current_source",
            "cpu_positive_cases": 576, "exact_negative_runs": 15, "actual_owner_abi_negative_controls": 54, "abi_resource_denial_controls": 12,
            "ranked_formal_descriptor_continuation": "normal checked source to inert LLVM/handoff",
            "runtime_abi_conditions_discharged": false,
            "protected_finalizer_admitted": false, "native_llvm_executed": false,
            "grants_artifact_or_launch_authority": false, "hardware_observed": false,
        }))
        .unwrap(),
    )
    .unwrap();
    eprintln!(
        "physical-entry actual source observations retained at {}",
        directory.display()
    );
}

#[test]
fn physical_entry_production_controls_reject_wrong_stage_or_feature() {
    for feature in &FEATURES[3..] {
        assert!(expected_rejection(feature, "generic unsupported KIR operation").is_err());
        assert!(expected_rejection(feature, "actual compiler process failed").is_err());
        for other in &FEATURES[3..] {
            if feature != other {
                assert!(expected_rejection(feature, rejection_fragment(other).unwrap()).is_err());
            }
        }
    }
    assert!(checked_feature("physical-entry-one-v20,physical-entry-diamond-v20").is_err());
    assert!(checked_feature("../other").is_err());
    assert!(checked_mode("native").is_err());
    assert!(expected_rejection(FEATURES[0], rejection_fragment(FEATURES[3]).unwrap()).is_err());
    assert!(serde_json::from_str::<PreparedInvocation>("{}").is_err());
}
