//! Actual Rust -> authenticated MIR39 -> pre-ranked exact KIR22 -> CPU/LLVM diagnostics.
//! The bounded parent prepares real dependencies once, then isolates each live
//! rustc session. No diagnostic reconstruction can mint this source owner.
//! Ranked/formal/descriptor continuation remains unavailable. Native LLVM,
//! protected artifact finalization and GPU execution are NOT run.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fe2o3_kernel_ir::{AccessMode, OperationKind, ScalarType, Type};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    SharedBufferV1, SimulationArgumentV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationTargetV1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticMirWireVersionV1, SemanticTerminatorKindV1};
use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::gfx942_inline_value_qualification_v30_tests::{
    checked, invocation_for_fixture, read_bounded, sanitized,
};
use super::{Callbacks, Compilation, Compiler, TyCtxt};

#[path = "gfx942_physical_lds_exchange_observation_v22_tests.rs"]
pub(super) mod observation;

const OUTPUT_ENV: &str = "FE2O3_TEST_PHYSICAL_LDS_EXCHANGE_OUTPUT_V22";
const CHILD_ENV: &str = "FE2O3_TEST_PHYSICAL_LDS_EXCHANGE_INPUTS_V22";
const FEATURE_ENV: &str = "FE2O3_TEST_PHYSICAL_LDS_EXCHANGE_FEATURE_V22";
const PACKAGE: &str = "fe2o3-production-extraction-fixture";
const CRATE_NAME: &str = "fe2o3_production_extraction_fixture";
const CHILD: &str = "production_rustc_driver_v1::gfx942_physical_lds_exchange_qualification_v22_tests::actual_physical_lds_exchange_source_child";
const PREFIX: &str = "FE2O3_PHYSICAL_LDS_EXCHANGE_OBSERVATION_V22 ";
const MODE_ENV: &str = "FE2O3_TEST_PHYSICAL_LDS_EXCHANGE_MODE_V22";
const MODES: [&str; 2] = ["observe", "diagnostic"];
const FEATURES: [&str; 19] = [
    "physical-lds-exchange-one-v22",
    "physical-lds-exchange-registers-v22",
    "physical-lds-exchange-wrong-launch-v22",
    "physical-lds-exchange-dynamic-grid-v22",
    "physical-lds-exchange-frame-base-v22",
    "physical-lds-exchange-frame-size-v22",
    "physical-lds-exchange-frame-alignment-v22",
    "physical-lds-exchange-frame-epoch-v22",
    "physical-lds-exchange-missing-write-wait-v22",
    "physical-lds-exchange-missing-barrier-v22",
    "physical-lds-exchange-missing-read-wait-v22",
    "physical-lds-exchange-wrong-peer-v22",
    "physical-lds-exchange-wrong-lds-address-v22",
    "physical-lds-exchange-wrong-store-v22",
    "physical-lds-exchange-missing-vm-v22",
    "physical-lds-exchange-wrong-carry-v22",
    "physical-lds-exchange-foreign-input-v22",
    "physical-lds-exchange-foreign-marker-v22",
    "physical-lds-exchange-mixed-marker-v22",
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
            "src/physical_lds_exchange_v22.rs",
            include_bytes!(
                "../../tests/fixtures/production-extraction-device/src/physical_lds_exchange_v22.rs"
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
        .ok_or("unknown or combined physical-lds-exchange feature")
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
        schema: "fe2o3-test-source-physical-lds-exchange-invocation-v22".into(),
        feature: feature.into(),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: source_hash(&fixture.join("src/physical_lds_exchange_v22.rs")),
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
        "physical-lds-exchange-wrong-launch-v22" | "physical-lds-exchange-dynamic-grid-v22" => {
            Ok("physical-lds-exchange requires exact authored launch128 and max_grid1")
        }
        "physical-lds-exchange-frame-base-v22"
        | "physical-lds-exchange-frame-size-v22"
        | "physical-lds-exchange-frame-alignment-v22"
        | "physical-lds-exchange-frame-epoch-v22" => {
            Ok("physical-lds-exchange requires exact static_u32_frame(0,512,4,1)")
        }
        // These intentionally remove a row. The actual block has31 operations,
        // so the block-size check precedes the declaration native-count check
        // and instruction-state checks. Same-count readiness mutants are unit tests.
        "physical-lds-exchange-missing-write-wait-v22"
        | "physical-lds-exchange-missing-barrier-v22"
        | "physical-lds-exchange-missing-read-wait-v22"
        | "physical-lds-exchange-missing-vm-v22" => Ok("LDS exchange block/operation bounds"),
        "physical-lds-exchange-wrong-peer-v22" => Ok("LDS exchange xor64 actual localX"),
        "physical-lds-exchange-wrong-lds-address-v22" => Ok("LDS exchange local write offset"),
        "physical-lds-exchange-wrong-store-v22" => {
            Ok("LDS exchange output requires exact ready peer LDS result")
        }
        "physical-lds-exchange-wrong-carry-v22" => Ok("LDS exchange pointer high provenance"),
        "physical-lds-exchange-foreign-input-v22" => {
            Ok("physical-lds-exchange begin requires exact root argument transport")
        }
        "physical-lds-exchange-foreign-marker-v22" => Ok("physical-lds-exchange excludes helpers"),
        "physical-lds-exchange-mixed-marker-v22" => {
            Ok("physical-lds-exchange terminal roster contains a foreign or excessive marker")
        }
        _ => Err("positive or unknown source must not be refused"),
    }
}
fn timely(elapsed: std::time::Duration, seconds: u64) -> Result<(), &'static str> {
    (elapsed < std::time::Duration::from_secs(seconds))
        .then_some(())
        .ok_or("monotonic qualification deadline")
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

// Failure-only diagnostics stay bounded and escaped by the panic formatter.
// Oversized text is never copied into the failure stream; admission stays closed.
fn rejection_diagnostic_for_failure(diagnostic: &str) -> &str {
    if diagnostic.len() <= 64 * 1024 {
        diagnostic
    } else {
        "<diagnostic omitted: exceeds64KiB>"
    }
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
            if !transaction.has_authenticated_physical_lds_exchange_v22() {
                return Err(
                    "actual physical-lds-exchange source marker was not authenticated".into(),
                );
            }
            let target = transaction
                .lower_physical_lds_exchange_diagnostic_v22()
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
#[ignore = "isolated actual AMD rustc child; use actual_physical_lds_exchange_source_ladder"]
fn actual_physical_lds_exchange_source_child() {
    let started = std::time::Instant::now();
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
        "diagnostic" => super::run_diagnostic_physical_lds_exchange_extraction_driver_v22(&actual.args, &output).map(|()| {
            let canonical=read_bounded(&output.join("canonical-v22.bin"),1024*1024).unwrap();
            let llvm=read_bounded(&output.join("canonical.ll"),64*1024).unwrap();
            let sidecar=read_bounded(&output.join("native-observation-input-v22.txt"),32*1024).unwrap();
            json!({"stage":"public_pre_ranked_diagnostic_driver","canonical_sha256":digest(&canonical),"llvm_sha256":digest(&llvm),"native_observation_sha256":digest(&sidecar)})
        }),
        _ => unreachable!(),
    };
    let observation = if FEATURES[..2].contains(&feature.as_str()) {
        result.unwrap()
    } else {
        let diagnostic = result.expect_err("invalid source unexpectedly admitted");
        expected_rejection(&feature, &diagnostic).unwrap_or_else(|reason| {
            panic!("exact source refusal mismatch: reason={reason}; feature={feature}; expected={:?}; actual={:?}",
                rejection_fragment(&feature),
                rejection_diagnostic_for_failure(&diagnostic));
        });
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
    assert_eq!(
        derive_record(&directory, &feature),
        actual,
        "source or invocation dependencies changed during session"
    );
    timely(started.elapsed(), 300).unwrap();
    let frame = serde_json::to_string(&json!({
        "schema": "fe2o3-test-source-physical-lds-exchange-observation-v22",
        "feature": feature, "mode": mode, "invocation": actual, "observation": observation,
        "actual_rustc_callback": true, "source_unchanged": true,
        "protected_finalizer_admitted": false, "native_llvm_executed": false,
        "grants_artifact_or_launch_authority": false, "hardware_observed": false,
    }))
    .unwrap();
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}

#[test]
#[ignore = "pinned-nightly real-source ladder; serialize Cargo and provide a fresh absolute output directory"]
fn actual_physical_lds_exchange_source_ladder() {
    let started = std::time::Instant::now();
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
            let session_started = std::time::Instant::now();
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
            require_current_source();
            timely(session_started.elapsed(), 300).unwrap();
            timely(started.elapsed(), 1200).unwrap();
            observations.push(observation);
        }
        if FEATURES[..2].contains(&feature) {
            for (file, cap) in [
                ("canonical-v22.bin", 1024 * 1024),
                ("canonical.ll", 64 * 1024),
                ("native-observation-input-v22.txt", 32 * 1024),
            ] {
                let observed = read_bounded(
                    &directory.join(format!("{feature}.observe")).join(file),
                    cap,
                )
                .unwrap();
                let public = read_bounded(
                    &directory.join(format!("{feature}.diagnostic")).join(file),
                    cap,
                )
                .unwrap();
                assert_eq!(
                    observed, public,
                    "public diagnostic output differs from same-source immutable pre-ranked owner"
                );
            }
        }
    }
    require_current_source();
    assert_eq!(observations.len(), 38);
    timely(started.elapsed(), 1200).unwrap();
    fs::write(
        directory.join("observation.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "fe2o3-test-source-physical-lds-exchange-ladder-v22", "observations": observations,
            "public_driver_output_relation": "exact_bytes_same_current_source",
            "cpu_positive_cases": 64, "cpu_observed_barrier_cases": 2, "cpu_exact_negative_cases": 12, "exact_negative_runs": 34,
            "ranked_formal_descriptor_continuation": "unavailable; pre-ranked diagnostics only",
            "protected_finalizer_admitted": false, "native_llvm_executed": false,
            "grants_artifact_or_launch_authority": false, "hardware_observed": false,
        }))
        .unwrap(),
    )
    .unwrap();
    timely(started.elapsed(), 1200).unwrap();
    eprintln!(
        "physical-lds-exchange actual source observations retained at {}",
        directory.display()
    );
    timely(started.elapsed(), 1200).unwrap();
}

#[test]
fn physical_lds_exchange_source_controls_reject_wrong_stage_or_feature() {
    for feature in &FEATURES[2..] {
        assert!(expected_rejection(feature, "generic unsupported KIR operation").is_err());
        assert!(expected_rejection(feature, "actual compiler process failed").is_err());
        for other in &FEATURES[2..] {
            if rejection_fragment(feature).unwrap() != rejection_fragment(other).unwrap() {
                assert!(expected_rejection(feature, rejection_fragment(other).unwrap()).is_err());
            }
        }
    }
    assert!(
        checked_feature("physical-lds-exchange-one-v22,physical-lds-exchange-registers-v22")
            .is_err()
    );
    assert!(checked_feature("../other").is_err());
    assert!(checked_mode("native").is_err());
    assert!(expected_rejection(FEATURES[0], rejection_fragment(FEATURES[2]).unwrap()).is_err());
    assert!(serde_json::from_str::<PreparedInvocation>("{}").is_err());
}

#[test]
fn physical_lds_exchange_positive_after_exact_deadline_is_refused() {
    use std::time::Duration;
    for bound in [300, 1200] {
        assert!(timely(Duration::from_secs(bound) - Duration::from_nanos(1), bound).is_ok());
        assert!(timely(Duration::from_secs(bound), bound).is_err());
        assert!(timely(Duration::from_secs(bound) + Duration::from_nanos(1), bound).is_err());
    }
}

#[test]
fn physical_lds_exchange_removed_rows_require_earlier_block_bound_not_later_native_count() {
    for feature in [
        "physical-lds-exchange-missing-write-wait-v22",
        "physical-lds-exchange-missing-barrier-v22",
        "physical-lds-exchange-missing-read-wait-v22",
        "physical-lds-exchange-missing-vm-v22",
    ] {
        assert_eq!(
            rejection_fragment(feature).unwrap(),
            "LDS exchange block/operation bounds"
        );
        assert!(
            expected_rejection(feature, "canonical: LDS exchange block/operation bounds").is_ok()
        );
        for other in [
            "LDS exchange exact initial32 native rows",
            "LDS exchange requires write LGKM completion",
            "generic unsupported KIR operation",
        ] {
            assert!(expected_rejection(feature, other).is_err());
        }
    }
}
#[test]
fn physical_lds_exchange_failure_diagnostic_observation_does_not_expand_admission() {
    let exact = "x".repeat(64 * 1024);
    assert_eq!(rejection_diagnostic_for_failure(&exact).len(), 64 * 1024);
    let oversized = exact + "x";
    assert_eq!(
        rejection_diagnostic_for_failure(&oversized),
        "<diagnostic omitted: exceeds64KiB>"
    );
    assert_eq!(
        expected_rejection("physical-lds-exchange-missing-write-wait-v22", &oversized),
        Err("diagnostic exceeds bound")
    );
}

#[test]
fn physical_lds_exchange_foreign_input_requires_actual_root_transport_rejection() {
    let feature = "physical-lds-exchange-foreign-input-v22";
    assert!(expected_rejection(feature, "physical-lds-exchange begin requires exact root argument transport in function metadata").is_ok());
    for other in [
        "physical-lds-exchange source contains a foreign argument assignment",
        "device code reaches a panic path",
    ] {
        assert!(expected_rejection(feature, other).is_err());
    }
}
