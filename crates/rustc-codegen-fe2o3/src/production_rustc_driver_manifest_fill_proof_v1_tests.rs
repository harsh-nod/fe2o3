//! Default manifest source regression and auxiliary opt-in protected effect proof.
//! Preparation may build dependencies. The protected parent/children never run Cargo.
use super::super::gfx942_inline_value_qualification_v30_tests::{
    checked, invocation_for_fixture_source, read_bounded, sanitized,
};
use super::*;
use crate::production_ranked_projection_v1::conditional_bound_observation_v1_tests as conditional;
use crate::production_reference_effect_join_v2::source_proof_freshness_v1 as proof;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

const PREPARE: &str = "FE2O3_TEST_MANIFEST_FILL_PREPARE_V1";
const INPUTS: &str = "FE2O3_TEST_MANIFEST_FILL_INPUTS_V1";
const CASE: &str = "FE2O3_TEST_MANIFEST_FILL_CASE_V1";
const RESULTS: &str = "FE2O3_TEST_MANIFEST_FILL_RESULTS_V1";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::manifest_fill_proof::actual_manifest_fill_signed_effect_child";
const PREFIX: &str = "FE2O3_MANIFEST_FILL_PROOF_V1 ";
const SOURCE: &[u8] = include_bytes!("../../../examples/fill/src/lib.rs");
const MANIFEST: &[u8] = include_bytes!("../../../examples/fill/Cargo.toml");
const CAP: usize = 16 * 1024 * 1024;
const PROOF_FEATURE: &str = "reference-proof";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Case {
    Original,
    WrongStore,
    Conditional,
}
impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::WrongStore => "wrong-store",
            Self::Conditional => "conditional",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "original" => Self::Original,
            "wrong-store" => Self::WrongStore,
            "conditional" => Self::Conditional,
            _ => panic!("unknown manifest-fill proof case"),
        }
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn package() -> PathBuf {
    workspace().join("examples/fill")
}

fn assert_source_current() {
    assert_eq!(
        read_bounded(&package().join("src/lib.rs"), 1024 * 1024).unwrap(),
        SOURCE
    );
    assert_eq!(
        read_bounded(&package().join("Cargo.toml"), 64 * 1024).unwrap(),
        MANIFEST
    );
}

fn digest(path: &Path, cap: usize) -> String {
    super::super::lower_hex_v1(&Sha256::digest(read_bounded(path, cap).unwrap()))
}

// A deliberately exact test mutation, not a source parser or production selector.
fn wrong_store_source(source: &[u8]) -> Vec<u8> {
    let source = std::str::from_utf8(source).unwrap();
    let needle = "*value = 42.5;";
    let (before, after) = source.split_once(needle).expect("one original GPU store");
    assert!(!after.contains(needle), "ambiguous GPU-store mutation");
    assert!(
        before.contains("*out = 42.5;"),
        "independent reference must remain intact"
    );
    format!("{before}*value = 43.5;{after}").into_bytes()
}

fn create_directory(path: &Path) {
    assert!(path.is_absolute());
    DirBuilder::new()
        .mode(0o700)
        .create(path)
        .expect("new private test directory");
    assert_eq!(
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

fn create_file(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    schema: String,
    selection: String,
    selected_features: Vec<String>,
    qualification_credit: bool,
    case: Case,
    source: PathBuf,
    source_sha256: String,
    original_source_sha256: String,
    package_manifest_sha256: String,
    tutorial_manifest_sha256: String,
    compiler_input: serde_json::Value,
    lock_sha256: String,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
}

fn default_manifest_input() -> serde_json::Value {
    let manifest_path = workspace().join("config/tutorial-kernel-manifest-v1.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&read_bounded(&manifest_path, CAP).unwrap()).unwrap();
    let rows: Vec<_> = manifest["compilerFixtures"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["fixtureId"] == "gfx942-fill-simulation")
        .collect();
    let [row] = rows.as_slice() else {
        panic!("exactly one fill manifest row");
    };
    assert_eq!(row["target"], "gfx942");
    let input = &row["compilerInput"];
    assert_eq!(input["packageManifest"], "examples/fill/Cargo.toml");
    assert_eq!(
        input["sourcePaths"],
        serde_json::json!(["examples/fill/src/lib.rs"])
    );
    assert_eq!(
        input["cargoTarget"],
        serde_json::json!({"kind":"lib", "name":"fe2o3_fill", "sourcePath":"src/lib.rs"})
    );
    assert_eq!(input["features"], serde_json::json!([]));
    assert_eq!(input["defaultFeatures"], true);
    assert_eq!(input["kernelSymbols"], serde_json::json!(["fill"]));
    input.clone()
}

fn has_exact_proof_feature(args: &[String]) -> bool {
    let feature = format!("feature=\"{PROOF_FEATURE}\"");
    let selected: Vec<_> = args
        .iter()
        .filter(|arg| arg.starts_with("feature=") || arg.starts_with("--cfg=feature="))
        .collect();
    selected == vec![&feature]
        && args
            .windows(2)
            .any(|pair| pair[0] == "--cfg" && pair[1] == feature)
}

fn derive_invocation(directory: &Path, case: Case) -> Invocation {
    assert_source_current();
    let original = package().join("src/lib.rs");
    let source = if case == Case::WrongStore {
        let path = directory.join("wrong-store.rs");
        assert_eq!(
            read_bounded(&path, 1024 * 1024).unwrap(),
            wrong_store_source(SOURCE)
        );
        path
    } else {
        original.clone()
    };
    let input = default_manifest_input();
    let metadata: serde_json::Value =
        serde_json::from_slice(&read_bounded(&directory.join("metadata.stdout"), CAP).unwrap())
            .unwrap();
    let packages: Vec<_> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            row["manifest_path"].as_str().map(Path::new)
                == Some(package().join("Cargo.toml").as_path())
        })
        .collect();
    let [selected_package] = packages.as_slice() else {
        panic!("exactly one actual fill package");
    };
    assert_eq!(
        selected_package["features"][PROOF_FEATURE],
        serde_json::json!([])
    );
    assert!(selected_package["features"].get("default").is_none());
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &package(),
        "fe2o3-fill",
        "fe2o3_fill",
        Some(PROOF_FEATURE),
        &source,
        "gfx942",
    );
    assert!(
        has_exact_proof_feature(&args),
        "explicit auxiliary proof cfg required"
    );
    let (_, default_binding, _) = invocation_for_fixture_source(
        directory,
        &package(),
        "fe2o3-fill",
        "fe2o3_fill",
        None,
        &original,
        "gfx942",
    );
    assert_ne!(
        crate_binding, default_binding,
        "proof selection cannot reuse default binding"
    );
    Invocation {
        schema: "fe2o3-test-manifest-fill-invocation-v1".into(),
        selection: "auxiliary-reference-proof-not-default-manifest-selection".into(),
        selected_features: vec![PROOF_FEATURE.into()],
        qualification_credit: false,
        case,
        source: source.clone(),
        source_sha256: digest(&source, 1024 * 1024),
        original_source_sha256: digest(&original, 1024 * 1024),
        package_manifest_sha256: digest(&package().join("Cargo.toml"), 64 * 1024),
        tutorial_manifest_sha256: digest(
            &workspace().join("config/tutorial-kernel-manifest-v1.json"),
            CAP,
        ),
        compiler_input: input,
        lock_sha256: digest(&workspace().join("Cargo.lock"), CAP),
        args,
        crate_binding,
        cargo_observation,
    }
}

fn preparation_record(directory: &Path) -> serde_json::Value {
    serde_json::json!({
        "schema": "fe2o3-test-manifest-fill-preparation-v1",
        "metadata_sha256": digest(&directory.join("metadata.stdout"), CAP),
        "dependencies_sha256": digest(&directory.join("dependencies.stdout"), CAP),
        "sysroot_sha256": digest(&directory.join("sysroot.stdout"), 4096),
        "validator_sha256": digest(&workspace().join("scripts/validate-tutorial-kernel-manifest.py"), CAP),
        "validation_sha256": digest(&directory.join("manifest-validation.stdout"), CAP),
        "host_source_sha256": digest(&package().join("src/main.rs"), 1024 * 1024),
        "reference_tests_sha256": digest(&package().join("tests/reference.rs"), 1024 * 1024),
        "original": derive_invocation(directory, Case::Original),
        "wrong_store": derive_invocation(directory, Case::WrongStore),
        "conditional": derive_invocation(directory, Case::Conditional),
        "proof_executed": false,
        "grants_artifact_or_launch_authority": false,
    })
}

fn checked_preparation(directory: &Path) -> serde_json::Value {
    assert!(directory.is_absolute());
    let recorded: serde_json::Value = serde_json::from_slice(
        &read_bounded(&directory.join("preparation.json"), 1024 * 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(
        recorded,
        preparation_record(directory),
        "stale or substituted preparation"
    );
    recorded
}

#[test]
#[ignore = "builds pinned offline AMD dependencies; run only under the primary's serialized build guard"]
fn prepare_actual_manifest_fill_inputs() {
    let directory = PathBuf::from(env::var_os(PREPARE).expect("new absolute preparation path"));
    create_directory(&directory);
    assert_source_current();
    let mut rustc = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
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
    let mut validator = Command::new("python3");
    checked(
        sanitized(&mut validator)
            .args(["-B", "scripts/validate-tutorial-kernel-manifest.py"])
            .current_dir(workspace()),
        &directory,
        "manifest-validation",
        None,
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
                "--features",
                PROOF_FEATURE,
                "--manifest-path",
            ])
            .arg(package().join("Cargo.toml")),
        &directory,
        "metadata",
        None,
    );
    let target = directory.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo)
        .args(["check", "--release", "--locked", "--offline", "-Zbuild-std=core", "-p", "fe2o3-device", "--target", "amdgcn-amd-amdhsa", "--message-format=json", "--manifest-path"])
        .arg(package().join("Cargo.toml")).arg("--target-dir").arg(&target)
        .env("CARGO_BUILD_JOBS", "1")
        .env("RUSTC", sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS", "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &directory, "dependencies", Some(&target));
    create_directory(&directory.join("analysis-output"));
    create_file(
        &directory.join("wrong-store.rs"),
        &wrong_store_source(SOURCE),
    );
    create_file(
        &directory.join("preparation.json"),
        &serde_json::to_vec_pretty(&preparation_record(&directory)).unwrap(),
    );
    checked_preparation(&directory);
}

struct CallbacksV1 {
    case: Case,
    calls: usize,
    result: Option<Result<SourceOutcomeV1, String>>,
}

struct SourceOutcomeV1 {
    observed: proof::Observation,
    diagnostic: String,
    conditional: Option<conditional::Observation>,
}

impl Callbacks for CallbacksV1 {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        assert_eq!(self.calls, 1);
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            if self.case == Case::Conditional {
                let ((result, pending), observed) = proof::observe(None, || {
                    conditional::observe(|| transaction.verify_general_kernel_checks())
                });
                use crate::production_pipeline::ProductionPipelineError as Pipeline;
                use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as Projection;
                if !matches!(
                    result,
                    Err(Pipeline::RankedProjection(Projection::Incomplete(
                        conditional::STOP
                    )))
                ) {
                    return Err(format!("not the exact post-bind stop: {:?}", result.err()));
                }
                let pending = pending?;
                assert_eq!(pending.kernel, "fill");
                assert_ne!(pending.canonical_digest, [0; 32]);
                assert_eq!((pending.selected, pending.signed_receipts), (1, 1));
                assert_eq!((pending.memory_effects, pending.value_expressions), (1, 1));
                assert_eq!(pending.pending_checks, 9);
                assert_eq!(pending.output.source_argument, 0);
                assert_eq!(pending.output.adjusted_argument, 0);
                assert_eq!(pending.output.physical_argument, 0);
                assert_eq!(pending.output.raw_reference_argument, 1);
                assert_eq!(pending.output.element_bytes, 4);
                assert_eq!(pending.output.address_domain, "GlobalLaunch");
                assert!(pending.work > 0);
                check_observation(self.case, &observed)?;
                Ok(SourceOutcomeV1 {
                    observed,
                    diagnostic: conditional::STOP.into(),
                    conditional: Some(pending),
                })
            } else {
                let (result, observed) =
                    proof::observe(None, || transaction.verify_general_kernel_checks());
                use crate::production_pipeline::ProductionPipelineError as Pipeline;
                use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as Projection;
                use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2 as Join;
                let diagnostic = match (self.case, result) {
                    (
                        Case::Original,
                        Err(Pipeline::RankedProjection(Projection::ReferenceEffectJoin(
                            Join::Compile(error),
                        ))),
                    ) => {
                        let diagnostic = error.to_string();
                        if !diagnostic.contains("FE2O3-OWN-002")
                            || !diagnostic.contains("launch dimension 0 is dynamic")
                        {
                            return Err(format!(
                                "wrong post-proof admission boundary: {diagnostic}"
                            ));
                        }
                        diagnostic
                    }
                    (
                        Case::WrongStore,
                        Err(Pipeline::RankedProjection(Projection::ReferenceEffectJoin(
                            Join::ProofExecution(error),
                        ))),
                    ) => {
                        if !is_semantic_assertion_failure(&error) {
                            return Err(format!("not a Verus semantic counterexample: {error}"));
                        }
                        error
                    }
                    (_, result) => {
                        return Err(format!(
                            "unexpected manifest-fill result: {:?}",
                            result.err()
                        ));
                    }
                };
                check_observation(self.case, &observed)?;
                Ok(SourceOutcomeV1 {
                    observed,
                    diagnostic,
                    conditional: None,
                })
            }
        })());
        Compilation::Stop
    }
}

fn is_semantic_assertion_failure(error: &str) -> bool {
    error.len() <= 64 * 1024
        && error.contains("UnexpectedProofResult")
        && error.contains("exit=Some(1), signal=None")
        && error.contains("verified, 1 errors\\n")
        && error.contains("assertion failed")
}

fn check_observation(case: Case, observed: &proof::Observation) -> Result<(), String> {
    if observed.request_count != 1 || observed.binding.is_none() {
        return Err(
            "exactly one current source effect request must reach the protected producer".into(),
        );
    }
    let positive = case != Case::WrongStore;
    if observed.normal_import_count != usize::from(positive)
        || observed.original_receipt.is_some() != positive
        || observed.stale_error.is_some()
        || observed.rejected_import_count != 0
        || observed.original_reimport_count != 0
    {
        return Err("wrong fresh protected proof/import count".into());
    }
    if let Some(receipt) = &observed.original_receipt {
        receipt.validate().map_err(str::to_owned)?;
    }
    Ok(())
}

#[test]
#[ignore = "prepared real-source child; requires admitted protected runtime, no Cargo or GPU"]
fn actual_manifest_fill_signed_effect_child() {
    let directory = PathBuf::from(env::var_os(INPUTS).expect("prepared manifest-fill inputs"));
    let before = checked_preparation(&directory);
    let case = Case::parse(&env::var(CASE).expect("explicit closed source case"));
    let actual = derive_invocation(&directory, case);
    assert!(has_exact_proof_feature(&actual.args));
    assert_eq!(env::var("CARGO_FEATURE_REFERENCE_PROOF").unwrap(), "1");
    assert_eq!(
        env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        actual.crate_binding
    );
    assert_eq!(
        env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        actual.cargo_observation
    );
    assert_eq!(
        env::var("CARGO_MANIFEST_DIR").unwrap(),
        package().to_str().unwrap()
    );
    assert_eq!(env::var("CARGO_PKG_NAME").unwrap(), "fe2o3-fill");
    assert_eq!(env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(env::var("CARGO_CRATE_NAME").unwrap(), "fe2o3_fill");
    let mut callbacks = CallbacksV1 {
        case,
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "actual rustc callback required");
    let outcome = callbacks
        .result
        .expect("source callback result")
        .expect("exact protected source proof boundary");
    assert_eq!(before, checked_preparation(&directory));
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none(),
        "no root artifact may escape"
    );
    let report = serde_json::json!({
        "schema": "fe2o3-test-manifest-fill-proof-v1", "case": case,
        "invocation": actual, "observed": outcome.observed,
        "diagnostic": outcome.diagnostic, "conditional": outcome.conditional,
        "actual_rustc_callback": true, "source_admission_complete": false,
        "default_manifest_selection": false, "qualification_credit": false,
        "grants_artifact_or_launch_authority": false, "hardware_observed": false,
    });
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(encoded.len() <= 64 * 1024);
    println!("\n{PREFIX}{encoded}");
}

#[test]
#[ignore = "prepared protected runtime integration; three fresh children, no Cargo or GPU"]
fn actual_manifest_fill_protected_effect_and_mutation() {
    let directory = PathBuf::from(env::var_os(INPUTS).expect("prepared manifest-fill inputs"));
    let results = PathBuf::from(env::var_os(RESULTS).expect("new private results path"));
    create_directory(&results);
    let before = checked_preparation(&directory);
    let mut reports = Vec::new();
    for case in [Case::Original, Case::WrongStore, Case::Conditional] {
        let actual = derive_invocation(&directory, case);
        let mut command = Command::new(env::current_exe().unwrap());
        sanitized(&mut command)
            .args([
                "--ignored",
                "--exact",
                CHILD,
                "--test-threads=1",
                "--nocapture",
            ])
            .env(INPUTS, &directory)
            .env(CASE, case.name())
            .env(CRATE_BINDING_ID_ENV_V1, &actual.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &actual.cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", package())
            .env("CARGO_PKG_NAME", "fe2o3-fill")
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", "fe2o3_fill")
            .env("CARGO_FEATURE_REFERENCE_PROOF", "1")
            .current_dir(workspace());
        let stdout = checked(&mut command, &results, case.name(), None);
        let text = std::str::from_utf8(&stdout).unwrap();
        assert!(
            text.contains("1 passed; 0 failed; 0 ignored"),
            "exactly one successful child test"
        );
        let records: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix(PREFIX))
            .collect();
        let [encoded] = records.as_slice() else {
            panic!("exactly one real proof observation required");
        };
        let report: serde_json::Value = serde_json::from_str(encoded).unwrap();
        assert_eq!(report["invocation"], serde_json::to_value(&actual).unwrap());
        assert_eq!(report["case"], serde_json::to_value(case).unwrap());
        assert_eq!(report["grants_artifact_or_launch_authority"], false);
        assert_eq!(report["source_admission_complete"], false);
        assert_eq!(report["default_manifest_selection"], false);
        assert_eq!(report["qualification_credit"], false);
        reports.push(report);
    }
    for field in ["kernel_mir", "normalized_obligation"] {
        assert_ne!(
            reports[0]["observed"]["binding"][field],
            reports[1]["observed"]["binding"][field]
        );
        assert_eq!(
            reports[0]["observed"]["binding"][field],
            reports[2]["observed"]["binding"][field]
        );
    }
    assert_eq!(
        reports[0]["invocation"]["source_sha256"],
        reports[2]["invocation"]["source_sha256"]
    );
    assert_ne!(
        reports[0]["invocation"]["source_sha256"],
        reports[1]["invocation"]["source_sha256"]
    );
    assert_eq!(before, checked_preparation(&directory));
    create_file(
        &results.join("report.json"),
        &serde_json::to_vec_pretty(&reports).unwrap(),
    );
}

#[test]
fn proof_feature_is_explicit_and_never_the_default_selection() {
    assert_eq!(default_manifest_input()["features"], serde_json::json!([]));
    let proof_args = ["--cfg", "feature=\"reference-proof\""].map(str::to_owned);
    assert!(has_exact_proof_feature(&proof_args));
    assert!(!has_exact_proof_feature(&[]));
    assert!(!has_exact_proof_feature(&[proof_args[1].clone()]));
    assert!(!has_exact_proof_feature(
        &[proof_args.clone(), proof_args].concat()
    ));
    assert!(!has_exact_proof_feature(
        &["--cfg", "feature=\"unannotated-fill\""].map(str::to_owned)
    ));
}

#[test]
#[ignore = "runs real no-gpu CLI and pinned offline source builds; primary serialized guard only"]
fn default_manifest_fill_quickstart_simulates_without_reference_proof() {
    assert_source_current();
    assert_eq!(default_manifest_input()["features"], serde_json::json!([]));
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-fill-quickstart");
    let temporary = scratch.path().join("tmp");
    create_directory(&temporary);
    let mut command = Command::new("bash");
    let result = checked(
        sanitized(&mut command)
            .arg(workspace().join("scripts/quickstart.sh"))
            .arg("no-gpu")
            .env("CARGO", env!("CARGO"))
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_NET_OFFLINE", "true")
            .env("TMPDIR", &temporary)
            .current_dir(workspace()),
        scratch.path(),
        "default-quickstart",
        None,
    );
    let result: serde_json::Value = serde_json::from_slice(&result).unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["authority"], "observation_only");
    assert_eq!(result["simulated"], true);
    assert_eq!(result["hardware_observed"], false);
    let expectation: serde_json::Value = serde_json::from_slice(
        &read_bounded(
            &workspace().join("scripts/quickstart/fill-canary-expectation.json"),
            CAP,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["arguments"], expectation["arguments"]);
    assert_eq!(result["shared_buffers"], expectation["shared_buffers"]);
    assert!(
        fs::read_dir(&temporary).unwrap().next().is_none(),
        "quickstart must clean its outputs"
    );
    assert_source_current();
}

#[test]
#[ignore = "real default manifest source with pinned AMD dependencies; no protected runtime or GPU"]
fn default_manifest_fill_reaches_checked_native_output() {
    assert_source_current();
    assert_eq!(default_manifest_input()["features"], serde_json::json!([]));
    ordinary_rust_checked_output_cases_for_profile(
        &[OrdinarySourceCase::Fill],
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    );
}

#[test]
fn mutation_changes_only_the_original_gpu_store_not_the_cpu_reference() {
    let changed = wrong_store_source(SOURCE);
    assert_eq!(changed.len(), SOURCE.len());
    let changes: Vec<_> = SOURCE
        .iter()
        .zip(&changed)
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changes, vec![(&b'2', &b'3')]);
    assert!(
        std::str::from_utf8(&changed)
            .unwrap()
            .contains("*out = 42.5;")
    );
}

#[test]
fn missing_runtime_and_absent_import_are_not_proof_success() {
    for message in [
        "proof runtime unavailable",
        "UnexpectedProofResult timeout",
        "assertion failed",
        "exit=Some(1), signal=None",
    ] {
        assert!(!is_semantic_assertion_failure(message));
    }
    for case in [Case::Original, Case::WrongStore, Case::Conditional] {
        assert!(check_observation(case, &proof::Observation::default()).is_err());
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and AMD dependencies; unsigned fixture is not the manifest fill"]
fn unannotated_fill_fixture_and_vecadd_reach_checked_native_output() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_checked_output_cases_for_profile(
            &[
                OrdinarySourceCase::UnannotatedFill,
                OrdinarySourceCase::Vecadd,
            ],
            profile,
        );
    }
}
