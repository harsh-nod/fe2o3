//! Actual-source protected proof observations, never a reference/proof bypass.
//! The host preparer records real Cargo artifacts. An isolated child rederives
//! the invocation and executes the normal source-to-ranked callback unchanged.
//! The positive currently proves its value relation, then fails TotalView
//! ownership; the two mutations must fail the real Verus assertion instead.

#[path = "gfx942_reference_stale_proof_v30_tests.rs"]
mod stale_proof;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::gfx942_inline_value_qualification_v30_tests::{
    checked, invocation_for_fixture, read_bounded, sanitized,
};
use super::{Callbacks, Compilation, Compiler, TyCtxt};

const PREPARE_ENV: &str = "FE2O3_TEST_ISA_REFERENCE_PREPARE_V30";
const CHILD_ENV: &str = "FE2O3_TEST_ISA_REFERENCE_INPUTS_V30";
const FEATURE_ENV: &str = "FE2O3_TEST_ISA_REFERENCE_FEATURE_V30";
const PACKAGE: &str = "fe2o3-production-extraction-fixture";
const CRATE_NAME: &str = "fe2o3_production_extraction_fixture";
const RECORD_SCHEMA: &str = "fe2o3-test-source-isa-reference-invocation-v30";
const OBSERVATION_PREFIX: &str = "FE2O3_ASSEMBLY_REFERENCE_OBSERVATION_V30 ";
const FEATURES: [&str; 3] = [
    "assembly-reference-positive",
    "assembly-reference-wrong-opcode",
    "assembly-reference-wrong-constant",
];
const BOUND: usize = 16 * 1024 * 1024;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/production-extraction-device")
        .canonicalize()
        .unwrap()
}

fn checked_feature(feature: &str) -> Result<&str, String> {
    FEATURES
        .contains(&feature)
        .then_some(feature)
        .ok_or_else(|| "unknown or combined source-reference feature".into())
}

fn source_hash(path: &Path) -> String {
    let bytes = read_bounded(path, 1024 * 1024).unwrap();
    super::lower_hex_v1(&Sha256::digest(bytes))
}

fn assert_compiled_source_is_current() {
    assert_eq!(
        read_bounded(&fixture().join("src/lib.rs"), 1024 * 1024).unwrap(),
        include_bytes!("../../tests/fixtures/production-extraction-device/src/lib.rs")
    );
    assert_eq!(
        read_bounded(
            &fixture().join("src/assembly_reference_v30.rs"),
            1024 * 1024
        )
        .unwrap(),
        include_bytes!(
            "../../tests/fixtures/production-extraction-device/src/assembly_reference_v30.rs"
        )
    );
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
}

fn derive_record(directory: &Path, feature: &str) -> PreparedInvocation {
    checked_feature(feature).unwrap();
    let fixture = fixture();
    let (args, crate_binding, cargo_observation) =
        invocation_for_fixture(directory, &fixture, PACKAGE, CRATE_NAME, Some(feature));
    PreparedInvocation {
        schema: RECORD_SCHEMA.into(),
        feature: feature.into(),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: source_hash(&fixture.join("src/assembly_reference_v30.rs")),
        root_source_sha256: source_hash(&fixture.join("src/lib.rs")),
        manifest_sha256: source_hash(&fixture.join("Cargo.toml")),
    }
}

fn expected_stage(feature: &str, result: &Result<(), String>) -> Result<&'static str, String> {
    checked_feature(feature)?;
    let Err(diagnostic) = result else {
        return Err("source unexpectedly passed the presently incomplete ownership gate".into());
    };
    if diagnostic.len() > 64 * 1024 {
        return Err("source callback diagnostic exceeds its bound".into());
    }
    if diagnostic.contains("proof runtime unavailable")
        || diagnostic.contains("source-to-proof V2 effect mismatch")
        || diagnostic.contains("source-to-proof V2 cannot normalize")
    {
        return Err("source/proof setup failure is not an acceptance observation".into());
    }
    if feature == FEATURES[0] {
        // CompilerOwnedReferenceEffectRequestV2::prove_and_compile executes and
        // imports every protected receipt before calling ranked admission.
        // This exact later diagnostic therefore establishes that stage order,
        // not final source admission and not a separately exposed proof receipt.
        if diagnostic.contains("source-to-proof V2 ranked admission failed: error[FE2O3-OWN-002]")
            && diagnostic.contains("launch dimension 0 is dynamic")
            && !diagnostic.contains("proof execution failed")
        {
            return Ok("protected_proof_completed_then_dynamic_total_view_refused");
        }
    } else if diagnostic.contains("functional-refinement proof execution failed:")
        && diagnostic.contains("UnexpectedProofResult")
        && diagnostic.contains("exit=Some(1), signal=None")
        && diagnostic.contains("verified, 1 errors\\n")
        && diagnostic.contains("assertion failed")
        && !diagnostic.contains("FE2O3-OWN-002")
    {
        return Ok("protected_verus_assertion_rejected_before_ownership");
    }
    Err(format!("unexpected actual source stage: {diagnostic}"))
}

#[derive(Default)]
struct ReferenceCallbacks {
    calls: usize,
    result: Option<Result<(), String>>,
}

impl Callbacks for ReferenceCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(super::extract_ranked_memory_in_active_session_v1(tcx, None));
        Compilation::Stop
    }
}

#[test]
#[ignore = "prepares real pinned-nightly Cargo artifacts outside the isolated proof container"]
fn prepare_actual_source_reference_inputs() {
    let directory = PathBuf::from(
        std::env::var_os(PREPARE_ENV).expect("provide a new task-owned preparation directory"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).expect("preparation directory must be new");
    assert_compiled_source_is_current();
    let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
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
    checked(
        sanitized(&mut cargo)
            .args(["check", "--release", "--locked", "--offline", "-Zbuild-std=core",
                "-p", "fe2o3-device", "--target", "amdgcn-amd-amdhsa", "--message-format=json",
                "--manifest-path"])
            .arg(fixture().join("Cargo.toml"))
            .arg("--target-dir").arg(&target)
            .env("RUSTC", sysroot.join("bin/rustc"))
            .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &directory,
        "dependencies",
        Some(&target),
    );
    fs::create_dir(directory.join("analysis-output")).unwrap();
    for feature in FEATURES {
        fs::write(
            directory.join(format!("{feature}.invocation.json")),
            serde_json::to_vec_pretty(&derive_record(&directory, feature)).unwrap(),
        )
        .unwrap();
    }
    assert_compiled_source_is_current();
    fs::write(
        directory.join("preparation.json"),
        serde_json::to_vec_pretty(&preparation_record(&directory)).unwrap(),
    )
    .unwrap();
    eprintln!(
        "actual source reference inputs retained at {}",
        directory.display()
    );
}

fn preparation_record(directory: &Path) -> serde_json::Value {
    json!({
        "schema": "fe2o3-test-source-isa-reference-preparation-v30",
        "features": FEATURES,
        "metadata_sha256": source_hash_bounded(&directory.join("metadata.stdout")),
        "artifacts_sha256": source_hash_bounded(&directory.join("dependencies.stdout")),
        "source_sha256": source_hash(&fixture().join("src/assembly_reference_v30.rs")),
        "root_source_sha256": source_hash(&fixture().join("src/lib.rs")),
        "manifest_sha256": source_hash(&fixture().join("Cargo.toml")),
        "rustc_callback_executed": false,
        "proof_executed": false,
        "grants_artifact_or_launch_authority": false,
    })
}

fn source_hash_bounded(path: &Path) -> String {
    super::lower_hex_v1(&Sha256::digest(read_bounded(path, BOUND).unwrap()))
}

#[test]
#[ignore = "isolated real-rustc child; requires prepared artifacts and the unchanged protected runtime"]
fn actual_source_reference_child() {
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("missing preparation directory"));
    let feature = std::env::var(FEATURE_ENV).expect("missing closed source feature");
    checked_feature(&feature).unwrap();
    assert_compiled_source_is_current();
    let preparation: serde_json::Value = serde_json::from_slice(
        &read_bounded(&directory.join("preparation.json"), 64 * 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(
        preparation,
        preparation_record(&directory),
        "invalid or stale preparation records"
    );
    let actual = derive_record(&directory, &feature);
    let retained: PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{feature}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, retained,
        "preparation bytes no longer match independent derivation"
    );
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
    let mut callbacks = ReferenceCallbacks::default();
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "exactly one real callback is required");
    let result = callbacks
        .result
        .expect("the actual callback was not reached");
    let stage = expected_stage(&feature, &result).unwrap();
    assert_compiled_source_is_current();
    assert!(
        !fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .any(|entry| entry.is_ok()),
        "the callback must stop before artifact emission"
    );
    println!("\n{OBSERVATION_PREFIX}{}", serde_json::to_string(&json!({
        "schema": "fe2o3-test-source-isa-reference-observation-v30",
        "feature": feature,
        "stage": stage,
        "actual_rustc_callback": true,
        "source_unchanged": true,
        "source_sha256": actual.source_sha256,
        "root_source_sha256": actual.root_source_sha256,
        "manifest_sha256": actual.manifest_sha256,
        "crate_binding": actual.crate_binding,
        "cargo_observation": actual.cargo_observation,
        "diagnostic": result.unwrap_err(),
        "proof_stage_evidence": if feature == FEATURES[0] {
            "normal protected receipt import precedes the ranked-admission diagnostic; no separate receipt exposed by this API"
        } else {
            "actual Verus assertion failure in the normal protected proof path, before ranked ownership admission"
        },
        "source_admission_complete": false,
        "grants_artifact_or_launch_authority": false,
        "hardware_observed": false,
    })).unwrap());
}

#[test]
fn source_reference_stage_controls_refuse_setup_and_false_success() {
    let positive = Err("source-to-proof V2 ranked admission failed: error[FE2O3-OWN-002]: GPU hierarchy ownership is incomplete because guarded invocation tracing failed: launch dimension 0 is dynamic".into());
    let negative = Err("functional-refinement proof execution failed: UnexpectedProofResult exit=Some(1), signal=None stdout=\"verification results:: 0 verified, 1 errors\\n\" stderr=\"assertion failed\"".into());
    assert!(expected_stage(FEATURES[0], &positive).is_ok());
    for feature in &FEATURES[1..] {
        assert!(expected_stage(feature, &negative).is_ok());
        assert!(expected_stage(feature, &positive).is_err());
    }
    assert!(expected_stage(FEATURES[0], &negative).is_err());
    for feature in FEATURES {
        assert!(expected_stage(feature, &Ok(())).is_err());
        for bad in [
            "functional-refinement proof runtime unavailable",
            "functional-refinement proof execution failed: TimedOut",
            "functional-refinement proof execution failed: UnexpectedProofResult exit=Some(1), signal=None syntax error",
            "source-to-proof V2 effect mismatch",
            "source-to-proof V2 cannot normalize the GPU store value",
        ] {
            assert!(expected_stage(feature, &Err(bad.into())).is_err());
        }
    }
    assert!(
        checked_feature("assembly-reference-positive,assembly-reference-wrong-opcode").is_err()
    );
    assert!(checked_feature("../other").is_err());
}

#[test]
fn prepared_invocation_rejects_unknown_and_missing_fields() {
    assert!(serde_json::from_str::<PreparedInvocation>("{}").is_err());
    assert!(serde_json::from_str::<PreparedInvocation>(
        r#"{"schema":"unexpected","feature":"x","args":[],"crate_binding":"","cargo_observation":"","source_sha256":"","root_source_sha256":"","manifest_sha256":"","extra":true}"#
    ).is_err());
}
