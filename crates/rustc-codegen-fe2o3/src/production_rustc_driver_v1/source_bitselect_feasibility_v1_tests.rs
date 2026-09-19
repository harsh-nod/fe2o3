//! Ignored first real-source HIR/semantic/KIR feasibility gate. No candidate
//! source writes, source admission, planner admission, proof or hardware claims.
//! Reuses the bounded existing Cargo/artifact harness; environment changes are
//! confined to child Command objects, never unsafe process-global mutations.

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

#[path = "source_bitselect_roundtrip_v1_tests.rs"]
mod roundtrip;

const OUTPUT_ENV: &str = "FE2O3_TEST_SOURCE_BITSELECT_OUTPUT";
const CHILD_ENV: &str = "FE2O3_TEST_SOURCE_BITSELECT_INPUTS";
const FEATURE_ENV: &str = "FE2O3_TEST_SOURCE_BITSELECT_FEATURE";
const PACKAGE: &str = "fe2o3-production-extraction-fixture";
const CRATE_NAME: &str = "fe2o3_production_extraction_fixture";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::actual_source_bitselect_child";
const PREFIX: &str = "FE2O3_SOURCE_BITSELECT_FEASIBILITY ";
const FEATURES: [&str; 4] = [
    "source-bitselect-feasibility",
    "source-bitselect-ambiguous",
    "source-bitselect-local-alias",
    "source-bitselect-normalized",
];
const FIXTURE_FILES: [(&str, &[u8]); 4] = [
    (
        "Cargo.toml",
        include_bytes!("../../tests/fixtures/production-extraction-device/Cargo.toml"),
    ),
    (
        "src/lib.rs",
        include_bytes!("../../tests/fixtures/production-extraction-device/src/lib.rs"),
    ),
    (
        "src/source_bitselect_feasibility.rs",
        include_bytes!(
            "../../tests/fixtures/production-extraction-device/src/source_bitselect_feasibility.rs"
        ),
    ),
    (
        "src/source_bitselect_normalized.rs",
        include_bytes!(
            "../../tests/fixtures/production-extraction-device/src/source_bitselect_normalized.rs"
        ),
    ),
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/production-extraction-device")
        .canonicalize()
        .unwrap()
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
    for (relative, expected) in FIXTURE_FILES {
        assert_eq!(
            read_bounded(&fixture().join(relative), 64 * 1024).unwrap(),
            expected,
            "rebuild feasibility harness after fixture change: {relative}"
        );
    }
    assert!(
        FIXTURE_FILES[3].1.starts_with(&[0xef, 0xbb, 0xbf]),
        "the normalized-source negative must retain its BOM"
    );
}

fn hash(path: &Path) -> String {
    super::lower_hex_v1(&Sha256::digest(
        read_bounded(path, 16 * 1024 * 1024).unwrap(),
    ))
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PreparedInvocation {
    feature: String,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
    fixture_sha256: [String; 4],
    artifacts_sha256: String,
    metadata_sha256: String,
}

fn derive_record(directory: &Path, feature: &str) -> PreparedInvocation {
    assert!(FEATURES.contains(&feature));
    let fixture = fixture();
    let (args, crate_binding, cargo_observation) =
        invocation_for_fixture(directory, &fixture, PACKAGE, CRATE_NAME, Some(feature));
    PreparedInvocation {
        feature: feature.into(),
        args,
        crate_binding,
        cargo_observation,
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture.join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}

fn expected_negative(feature: &str) -> Option<&'static str> {
    match feature {
        "source-bitselect-ambiguous" => Some("source-boundary ambiguous bitselect initializers"),
        "source-bitselect-local-alias" => Some("source-boundary local alias is not a parameter"),
        "source-bitselect-normalized" => {
            Some("source-boundary normalization changes original offsets")
        }
        _ => None,
    }
}

#[derive(Default)]
struct SourceCallbacks {
    calls: usize,
    result: Option<Result<Value, String>>,
}

impl Callbacks for SourceCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(
            super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| transaction.observe_source_bitselect_feasibility()),
        );
        Compilation::Stop
    }
}

#[test]
#[ignore = "isolated real rustc child; use actual_source_bitselect_feasibility_ladder"]
fn actual_source_bitselect_child() {
    let directory = PathBuf::from(std::env::var_os(CHILD_ENV).expect("preparation directory"));
    let feature = std::env::var(FEATURE_ENV).expect("feature");
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
    let mut callbacks = SourceCallbacks::default();
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(
        callbacks.calls, 1,
        "actual compiler callback must execute once"
    );
    let result = callbacks
        .result
        .expect("actual callback did not reach qualification");
    let observation = if feature == FEATURES[0] {
        let observed = result.unwrap();
        assert_eq!(observed["stage"], "actual_typed_hir_semantic_kir_join");
        assert_eq!(observed["parameters"].as_array().unwrap().len(), 3);
        assert_eq!(
            observed["kernel_ir_operations"].as_array().unwrap().len(),
            3
        );
        assert_eq!(observed["normal_stages_and_equivalence_replayed"], true);
        assert_eq!(observed["replacement_admitted"], false);
        observed
    } else {
        let diagnostic = result.expect_err("negative source must not complete its HIR join");
        assert_eq!(
            diagnostic,
            expected_negative(&feature).expect("known exact negative"),
            "a later compiler failure is not evidence for this source boundary"
        );
        json!({"stage": "actual_source_boundary_refused", "diagnostic": diagnostic})
    };
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let output = serde_json::to_vec(&json!({
        "feature": feature, "invocation": actual, "observation": observation,
        "actual_rustc_callback": true, "source_unchanged": true,
        "candidate_written": false, "replacement_admitted": false,
        "grants_artifact_or_launch_authority": false, "hardware_observed": false,
    }))
    .unwrap();
    assert!(output.len() <= 64 * 1024, "bounded diagnostic output");
    println!("\n{PREFIX}{}", std::str::from_utf8(&output).unwrap());
}

#[test]
#[ignore = "pinned-nightly real-source gate; serialize Cargo and provide a fresh absolute output directory"]
fn actual_source_bitselect_feasibility_ladder() {
    let directory = PathBuf::from(
        std::env::var_os(OUTPUT_ENV).expect("set a fresh absolute task-owned output directory"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    require_current_source();
    let rustc_path = PathBuf::from(
        std::env::var_os("RUSTC").expect("set RUSTC to the absolute pinned-nightly binary"),
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
    let mut observations = Vec::with_capacity(FEATURES.len());
    for feature in FEATURES {
        let record = derive_record(&directory, feature);
        fs::write(
            directory.join(format!("{feature}.invocation.json")),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap());
        let stdout = checked(
            sanitized(&mut child)
                .current_dir(repository())
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(CHILD_ENV, &directory)
                .env(FEATURE_ENV, feature)
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
            feature,
            None,
        );
        let stdout = std::str::from_utf8(&stdout).unwrap();
        let lines = stdout
            .lines()
            .filter_map(|line| line.strip_prefix(PREFIX))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= 64 * 1024);
        assert_eq!(
            stdout
                .lines()
                .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let observation: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(observation["feature"], feature);
        assert_eq!(
            observation["invocation"],
            serde_json::to_value(record).unwrap()
        );
        observations.push(observation);
    }
    require_current_source();
    let report = serde_json::to_vec_pretty(&json!({
        "observations": observations, "actual_rustc_callback": true,
        "candidate_written": false, "replacement_admitted": false,
        "grants_artifact_or_launch_authority": false, "hardware_observed": false,
    }))
    .unwrap();
    assert!(report.len() <= 512 * 1024);
    fs::write(directory.join("observation.json"), report).unwrap();
    eprintln!(
        "first source-boundary feasibility observations: {}",
        directory.display()
    );
}

#[test]
fn source_bitselect_negatives_are_exact_stage_specific() {
    assert_eq!(expected_negative("source-bitselect-feasibility"), None);
    assert_eq!(expected_negative("unknown"), None);
    for feature in &FEATURES[1..] {
        let expected = expected_negative(feature).unwrap();
        assert!(expected.starts_with("source-boundary "));
        assert_ne!(expected, "source-boundary materialization: unavailable");
    }
}
