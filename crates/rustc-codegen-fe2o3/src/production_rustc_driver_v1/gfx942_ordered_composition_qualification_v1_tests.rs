//! Fifteen isolated actual Rust sessions for the explicit composition importer.
//! Diagnostic source/KIR/LLVM and CPU observations only, not normal handoff,
//! a numerical proof, native execution, source publication or launch authority.
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

#[path = "gfx942_ordered_composition_cpu_v1_tests.rs"]
mod cpu;
#[path = "gfx942_ordered_composition_inputs_v1_tests.rs"]
mod inputs;
#[path = "gfx942_ordered_composition_normal_v1_tests.rs"]
mod normal;
#[path = "gfx942_ordered_composition_observation_v1_tests.rs"]
mod observation;

const OUTPUT_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_OUTPUT_V1";
const CHILD_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_INPUTS_V1";
const FEATURE_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_FEATURE_V1";
const PACKAGE: &str = "fe2o3-production-extraction-fixture";
const CRATE_NAME: &str = "fe2o3_production_extraction_fixture";
const CHILD: &str = "production_rustc_driver_v1::gfx942_ordered_composition_qualification_v1_tests::actual_ordered_composition_child";
const PREFIX: &str = "FE2O3_ORDERED_COMPOSITION_OBSERVATION_V1 ";
const FEATURES: [&str; 15] = [
    "ordered-composition-root",
    "ordered-composition-helper",
    "ordered-composition-two-calls",
    "ordered-composition-root-helper",
    "ordered-composition-const-monos",
    "ordered-composition-scalar-helper",
    "ordered-composition-wrapping",
    "ordered-composition-nested",
    "ordered-composition-conditional",
    "ordered-composition-wrong-abi",
    "ordered-composition-foreign-marker",
    "ordered-composition-too-many",
    "ordered-composition-wrong-launch",
    "ordered-composition-dynamic-register",
    "ordered-composition-mixed-marker",
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
fn digest(bytes: &[u8]) -> String {
    super::lower_hex_v1(&Sha256::digest(bytes))
}
fn publish_json(directory: &Path, name: &str, value: &impl Serialize) {
    let bytes = serde_json::to_vec_pretty(value).unwrap();
    super::publish_new_inert_output(
        &directory.join(name),
        &bytes,
        16 * 1024 * 1024,
        "composition qualification JSON",
    )
    .unwrap();
}
fn checked_feature(feature: &str) -> Result<(), &'static str> {
    FEATURES
        .contains(&feature)
        .then_some(())
        .ok_or("unknown or combined composition feature")
}
fn rejection_fragment(feature: &str) -> Result<&'static str, &'static str> {
    match feature {
        "ordered-composition-nested" => {
            Ok("ordered composition helper only calls authenticated program markers")
        }
        "ordered-composition-conditional" => {
            Ok("ordered program requires bounded unconditional acyclic source placement")
        }
        "ordered-composition-wrong-abi" | "ordered-composition-foreign-marker" => {
            Ok("ordered composition helper requires exact Direct Rust u32 ABI")
        }
        "ordered-composition-too-many" => Ok("ordered composition source roster bounds exceeded"),
        "ordered-composition-wrong-launch" => {
            Ok("ordered composition requires required and maximum 64x1x1 workgroup bounds")
        }
        "ordered-composition-dynamic-register" => {
            Ok("ordered program physical role is not an actual MIR constant")
        }
        "ordered-composition-mixed-marker" => {
            Ok("ordered composition mixes another assembly source family")
        }
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
fn timely(elapsed: std::time::Duration, seconds: u64) -> Result<(), &'static str> {
    (elapsed < std::time::Duration::from_secs(seconds))
        .then_some(())
        .ok_or("monotonic qualification deadline")
}
struct BodyCallbacks<'a> {
    feature: &'a str,
    output: &'a Path,
    started: std::time::Instant,
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
            let target = transaction.lower_ordered_composition_diagnostic_v1()?;
            observation::observe(target, self.feature, self.output, self.started)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "isolated actual AMD rustc child; use actual_ordered_composition_ladder"]
fn actual_ordered_composition_child() {
    let started = std::time::Instant::now();
    let directory = PathBuf::from(std::env::var_os(CHILD_ENV).expect("preparation directory"));
    assert!(directory.is_absolute());
    let feature = std::env::var(FEATURE_ENV).expect("feature");
    checked_feature(&feature).unwrap();
    let actual = inputs::derive_record(&directory, &feature);
    let retained: inputs::PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{feature}.invocation.json")),
            128 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, retained,
        "stale source, metadata, dependencies or invocation"
    );
    for (key, value) in [
        (CRATE_BINDING_ID_ENV_V1, actual.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            actual.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", PACKAGE),
        ("CARGO_PKG_VERSION", "0.1.0"),
        ("CARGO_CRATE_NAME", CRATE_NAME),
    ] {
        assert_eq!(std::env::var(key).unwrap(), value);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        fixture()
    );
    super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let output = directory.join(format!("{feature}.observe"));
    let mut callbacks = BodyCallbacks {
        feature: &feature,
        output: &output,
        started,
        calls: 0,
        result: None,
    };
    timely(started.elapsed(), 300).unwrap();
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(
        callbacks.calls, 1,
        "expected exactly one live rustc callback"
    );
    let result = callbacks.result.expect("actual callback was not reached");
    // Retain actual error before matching it. An unknown earlier rejection is
    // not a success and must be diagnosed without losing its concrete witness.
    if let Err(error) = &result {
        assert!(error.len() <= 64 * 1024);
        publish_json(
            &directory,
            &format!("{feature}.rejection.json"),
            &json!({
                "schema":"fe2o3-test-ordered-composition-rejection-v1",
                "feature":feature,"diagnostic":error,"exact_boundary_accepted":false,
            }),
        );
    }
    let observed = if FEATURES[..7].contains(&feature.as_str()) {
        result.unwrap()
    } else {
        let diagnostic = result.expect_err("invalid source unexpectedly admitted");
        expected_rejection(&feature, &diagnostic).unwrap();
        assert!(
            !output.exists(),
            "rejected source cannot publish diagnostics"
        );
        json!({"stage":"exact_source_composition_refused","diagnostic":diagnostic})
    };
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(
        inputs::derive_record(&directory, &feature),
        actual,
        "source or dependency contents changed during the session"
    );
    timely(started.elapsed(), 300).unwrap();
    let frame = serde_json::to_string(&json!({
        "schema":"fe2o3-test-ordered-composition-observation-v1",
        "feature":feature,"invocation":actual,"observation":observed,
        "actual_rustc_callbacks":callbacks.calls,"source_and_dependencies_unchanged":true,
        "normal_ranked_formal_handoff_qualified":false,
        "source_publication_attempted":false,"native_llvm_executed":false,
        "grants_artifact_or_launch_authority":false,"hardware_observed":false,
    }))
    .unwrap();
    assert!(frame.len() <= 128 * 1024);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
#[test]
#[ignore = "pinned-nightly live source ladder; serialize Cargo and supply a fresh absolute output"]
fn actual_ordered_composition_ladder() {
    let started = std::time::Instant::now();
    let directory =
        PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh task-owned output directory"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let source_before = inputs::current_sources();
    let rustc_path = PathBuf::from(
        std::env::var_os("RUSTC").expect("absolute pinned-nightly RUSTC; no manager auto-install"),
    );
    assert!(rustc_path.is_absolute());
    let mut rustc = Command::new(rustc_path);
    let bytes = checked(
        sanitized(&mut rustc).args(["--print", "sysroot"]),
        &directory,
        "sysroot",
        None,
    );
    let sysroot = PathBuf::from(std::str::from_utf8(&bytes).unwrap().trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    timely(started.elapsed(), 1200).unwrap();
    let mut metadata = Command::new(sysroot.join("bin/cargo"));
    let bytes = checked(
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
    let metadata: Value = serde_json::from_slice(&bytes).unwrap();
    for feature in FEATURES {
        inputs::feature_in_metadata(&metadata, feature).unwrap();
    }
    timely(started.elapsed(), 1200).unwrap();
    let dependency_target = directory.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo).current_dir(repository()).args([
        "check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device",
        "--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path",
    ]).arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&dependency_target)
        .env("RUSTC",sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &directory,"dependencies",Some(&dependency_target));
    timely(started.elapsed(), 1200).unwrap();
    fs::create_dir(directory.join("analysis-output")).unwrap();
    let (dependency_before, files) = inputs::dependency_snapshot(&directory);
    publish_json(&directory, "dependency-files.json", &files);
    drop(files);
    assert_eq!(inputs::current_sources(), source_before);
    let mut observations = Vec::new();
    for feature in FEATURES {
        let session_started = std::time::Instant::now();
        let record = inputs::derive_record(&directory, feature);
        assert_eq!(record.sources, source_before);
        assert_eq!(record.dependencies, dependency_before);
        publish_json(&directory, &format!("{feature}.invocation.json"), &record);
        let mut child = Command::new(std::env::current_exe().unwrap());
        timely(session_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
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
        // The reused helper has bounded streams/wait/drain. This separate
        // monotonic fence includes closure, decoding, validation and rehashing.
        timely(session_started.elapsed(), 300).unwrap();
        let stdout = std::str::from_utf8(&stdout).unwrap();
        let frames = stdout
            .lines()
            .filter_map(|line| line.strip_prefix(PREFIX))
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 1);
        assert_eq!(
            stdout
                .lines()
                .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let observed: Value = serde_json::from_str(frames[0]).unwrap();
        assert_eq!(
            observed["schema"],
            "fe2o3-test-ordered-composition-observation-v1"
        );
        assert_eq!(observed["feature"], feature);
        assert_eq!(
            observed["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observed["actual_rustc_callbacks"], 1);
        assert_eq!(observed["source_and_dependencies_unchanged"], true);
        assert_eq!(inputs::derive_record(&directory, feature), record);
        if FEATURES[..7].contains(&feature) {
            assert_eq!(observed["observation"]["cpu"]["cases"], 64);
            let output = directory.join(format!("{feature}.observe"));
            for (name, field) in [
                ("canonical-v17.bin", "canonical_bytes_sha256"),
                ("canonical.ll", "llvm_sha256"),
            ] {
                let bytes = read_bounded(&output.join(name), 256 * 1024).unwrap();
                assert_eq!(observed["observation"][field], digest(&bytes));
            }
        } else {
            assert_eq!(
                observed["observation"]["stage"],
                "exact_source_composition_refused"
            );
            expected_rejection(
                feature,
                observed["observation"]["diagnostic"].as_str().unwrap(),
            )
            .unwrap();
            assert!(!directory.join(format!("{feature}.observe")).exists());
        }
        timely(session_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        publish_json(&directory, &format!("{feature}.accepted.json"), &observed);
        timely(session_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        observations.push(observed);
    }
    assert_eq!(observations.len(), 15);
    assert_eq!(inputs::current_sources(), source_before);
    assert_eq!(inputs::dependency_snapshot(&directory).0, dependency_before);
    let cpu_cases: u64 = observations
        .iter()
        .filter_map(|o| o["observation"]["cpu"]["cases"].as_u64())
        .sum();
    assert_eq!(cpu_cases, 448);
    let exact_negatives = observations
        .iter()
        .filter(|o| o["observation"]["stage"] == "exact_source_composition_refused")
        .count();
    assert_eq!(exact_negatives, 8);
    let report = json!({
        "schema":"fe2o3-test-ordered-composition-source-ladder-v1",
        "actual_rustc_sessions":observations.len(),"cpu_cases":cpu_cases,
        "exact_source_negative_sessions":exact_negatives,"observations":observations,
        "source_files":source_before,"dependency_snapshot":dependency_before,
        "fresh_dependency_builds":1,"normal_ranked_formal_handoff_qualified":false,
        "source_publication_attempted":false,"native_llvm_executed":false,
        "grants_artifact_or_launch_authority":false,"hardware_observed":false,
        "cleanup_scope":"bounded direct-child/process-group helper; not whole-family supervision",
        "acceptance":"requires completed successful parent test, not this historical JSON alone",
    });
    timely(started.elapsed(), 1200).unwrap();
    publish_json(&directory, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
    eprintln!(
        "composition actual-source observations retained at {}",
        directory.display()
    );
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
fn composition_source_controls_reject_wrong_stage_or_feature() {
    for feature in &FEATURES[7..] {
        for diagnostic in [
            "generic unsupported KIR operation",
            "compiler process failed",
            "unrelated panic",
        ] {
            assert!(expected_rejection(feature, diagnostic).is_err());
        }
        for other in &FEATURES[7..] {
            if rejection_fragment(feature).unwrap() != rejection_fragment(other).unwrap() {
                assert!(expected_rejection(feature, rejection_fragment(other).unwrap()).is_err());
            }
        }
    }
    assert!(checked_feature("ordered-composition-root,ordered-composition-helper").is_err());
    assert!(checked_feature("../another").is_err());
    assert!(expected_rejection(FEATURES[0], rejection_fragment(FEATURES[7]).unwrap()).is_err());
    assert!(serde_json::from_str::<inputs::PreparedInvocation>("{}").is_err());
}
#[test]
fn composition_source_late_positive_is_refused() {
    use std::time::Duration;
    for bound in [300, 1200] {
        assert!(timely(Duration::from_secs(bound) - Duration::from_nanos(1), bound).is_ok());
        assert!(timely(Duration::from_secs(bound), bound).is_err());
        assert!(timely(Duration::from_secs(bound) + Duration::from_nanos(1), bound).is_err());
    }
}

#[path = "gfx942_ordered_composition_publish_qualification_v1_tests.rs"]
mod publisher;
