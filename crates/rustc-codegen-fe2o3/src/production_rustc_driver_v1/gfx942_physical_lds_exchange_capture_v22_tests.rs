//! Ignored real-source qualifier; reuses completed D3 preparation without building.
//! Canonical exports are byte-comparison inputs, never source-owner constructors.
use super::super::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, CRATE_BINDING_ID_ENV_V1, CRATE_NAME, Callbacks,
    Compilation, Compiler, FEATURES, PACKAGE, PreparedInvocation, TyCtxt, checked, derive_record,
    fixture, read_bounded, repository, require_current_source, sanitized, timely,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

#[path = "gfx942_physical_lds_exchange_capture_checks_v22_tests.rs"]
mod checks;

const SOURCE_ENV: &str = "FE2O3_TEST_PHYSICAL_LDS_CAPTURE_SOURCE_V22";
const OUTPUT_ENV: &str = "FE2O3_TEST_PHYSICAL_LDS_CAPTURE_OUTPUT_V22";
const FEATURE_ENV: &str = "FE2O3_TEST_PHYSICAL_LDS_CAPTURE_FEATURE_V22";
const PREFIX: &str = "FE2O3_PHYSICAL_LDS_CAPTURE_OBSERVATION_V22 ";
const CHILD: &str = "production_rustc_driver_v1::gfx942_physical_lds_exchange_qualification_v22_tests::observation::capture::actual_physical_lds_capture_child";
const OBSERVATION_CAP: usize = 2 * 1024 * 1024;
const FRAME_CAP: usize = 64 * 1024;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Inputs {
    schema: String,
    feature: String,
    invocation: PreparedInvocation,
    source_observation_sha256: String,
    canonical_sha256: String,
    canonical_bytes: usize,
    llvm_sha256: String,
    llvm_bytes: usize,
}
fn digest(bytes: &[u8]) -> String {
    crate::production_rustc_driver_v1::lower_hex_v1(&Sha256::digest(bytes))
}
fn positive_feature(feature: &str) -> Result<(), &'static str> {
    FEATURES[..2]
        .contains(&feature)
        .then_some(())
        .ok_or("capture requires exact one/register source")
}
fn export_join(
    observation: &Value,
    feature: &str,
    record: &PreparedInvocation,
    canonical: &[u8],
    llvm: &[u8],
) -> Result<(), &'static str> {
    positive_feature(feature)?;
    if observation["schema"] != "fe2o3-test-source-physical-lds-exchange-ladder-v22" {
        return Err("wrong D3 observation schema");
    }
    let rows = observation["observations"]
        .as_array()
        .ok_or("missing D3 observations")?;
    if rows.len() != 38 {
        return Err("incomplete D3 source ladder");
    }
    let mut rows = rows
        .iter()
        .filter(|r| r["feature"] == feature && r["mode"] == "diagnostic");
    let row = rows.next().ok_or("missing exact diagnostic row")?;
    if rows.next().is_some() {
        return Err("duplicate diagnostic row");
    }
    if row["schema"] != "fe2o3-test-source-physical-lds-exchange-observation-v22"
        || row["invocation"] != serde_json::to_value(record).unwrap()
        || row["actual_rustc_callback"] != true
        || row["source_unchanged"] != true
        || row["protected_finalizer_admitted"] != false
        || row["hardware_observed"] != false
        || row["grants_artifact_or_launch_authority"] != false
        || row["observation"]["stage"] != "public_pre_ranked_diagnostic_driver"
        || row["observation"]["canonical_sha256"] != digest(canonical)
        || row["observation"]["llvm_sha256"] != digest(llvm)
    {
        return Err("D3 exact source/export relation changed");
    }
    Ok(())
}
fn exports(directory: &Path, feature: &str) -> (Vec<u8>, Vec<u8>) {
    positive_feature(feature).unwrap();
    let path = directory.join(format!("{feature}.diagnostic"));
    (
        read_bounded(&path.join("canonical-v22.bin"), 1024 * 1024).unwrap(),
        read_bounded(&path.join("canonical.ll"), 64 * 1024).unwrap(),
    )
}
fn inputs(directory: &Path, feature: &str) -> Inputs {
    positive_feature(feature).unwrap();
    require_current_source();
    let invocation = derive_record(directory, feature);
    let old: PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{feature}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(old, invocation, "D3 preparation changed");
    let observation = read_bounded(&directory.join("observation.json"), OBSERVATION_CAP).unwrap();
    let parsed: Value = serde_json::from_slice(&observation).unwrap();
    let (canonical, llvm) = exports(directory, feature);
    export_join(&parsed, feature, &invocation, &canonical, &llvm).unwrap();
    Inputs {
        schema: "fe2o3-test-source-lds-capture-inputs-v22".into(),
        feature: feature.into(),
        invocation,
        source_observation_sha256: digest(&observation),
        canonical_sha256: digest(&canonical),
        canonical_bytes: canonical.len(),
        llvm_sha256: digest(&llvm),
        llvm_bytes: llvm.len(),
    }
}
struct BodyCallbacks<'a> {
    feature: &'a str,
    canonical: &'a [u8],
    llvm: &'a [u8],
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for BodyCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            let transaction = crate::production_rustc_driver_v1::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            if !transaction.has_authenticated_physical_lds_exchange_v22() {
                return Err("actual V22 source marker missing".into());
            }
            let target = transaction
                .lower_physical_lds_exchange_diagnostic_v22()
                .map_err(|e| e.to_string())?;
            Ok(checks::observe(
                target,
                self.feature,
                self.canonical,
                self.llvm,
            ))
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "isolated genuine rustc child; use actual_physical_lds_capture_ladder"]
fn actual_physical_lds_capture_child() {
    let started = Instant::now();
    let source = PathBuf::from(std::env::var_os(SOURCE_ENV).expect("D3 source directory"));
    let output = PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("capture output directory"));
    let feature = std::env::var(FEATURE_ENV).expect("exact positive feature");
    assert!(source.is_absolute() && output.is_absolute());
    positive_feature(&feature).unwrap();
    let actual = inputs(&source, &feature);
    let expected: Inputs = serde_json::from_slice(
        &read_bounded(&output.join(format!("{feature}.inputs.json")), FRAME_CAP).unwrap(),
    )
    .unwrap();
    assert_eq!(actual, expected, "source/export inputs changed");
    let invocation = &actual.invocation;
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        invocation.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        invocation.cargo_observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    crate::production_rustc_driver_v1::require_canonical_overflow_checks_v1(&invocation.args)
        .unwrap();
    let (canonical, llvm) = exports(&source, &feature);
    let mut callback = BodyCallbacks {
        feature: &feature,
        canonical: &canonical,
        llvm: &llvm,
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&invocation.args, &mut callback);
    assert_eq!(callback.calls, 1);
    let observation = callback.result.expect("live compiler callback").unwrap();
    require_current_source();
    assert_eq!(inputs(&source, &feature), actual);
    assert!(
        fs::read_dir(source.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    timely(started.elapsed(), 300).unwrap();
    let frame=serde_json::to_string(&json!({
        "schema":"fe2o3-test-source-lds-capture-observation-v22","inputs":actual,"observation":observation,
        "actual_rustc_callback":true,"canonical_export_relation":"unchanged_bytes_from_new_live_source_owner",
        "public_v22_cli":"unavailable","source_custody_from_bytes":false,"hardware_observed":false,
        "native_execution":false,"grants_artifact_or_launch_authority":false
    })).unwrap();
    assert!(frame.len() <= FRAME_CAP);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
#[test]
#[ignore = "requires completed unchanged D3 source ladder; two isolated live compiler callbacks, no dependency build"]
fn actual_physical_lds_capture_ladder() {
    let started = Instant::now();
    let source =
        PathBuf::from(std::env::var_os(SOURCE_ENV).expect("completed D3 source ladder directory"));
    let output =
        PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh capture output directory"));
    assert!(source.is_absolute() && output.is_absolute());
    assert_ne!(source, output);
    fs::create_dir(&output).unwrap();
    require_current_source();
    let mut observations = Vec::new();
    for feature in &FEATURES[..2] {
        let expected = inputs(&source, feature);
        let bytes = serde_json::to_vec_pretty(&expected).unwrap();
        assert!(bytes.len() <= FRAME_CAP);
        fs::write(output.join(format!("{feature}.inputs.json")), bytes).unwrap();
        let child_started = Instant::now();
        let mut child = Command::new(std::env::current_exe().unwrap());
        let stdout = checked(
            sanitized(&mut child)
                .current_dir(repository())
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(SOURCE_ENV, &source)
                .env(OUTPUT_ENV, &output)
                .env(FEATURE_ENV, feature)
                .env(CRATE_BINDING_ID_ENV_V1, &expected.invocation.crate_binding)
                .env(
                    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                    &expected.invocation.cargo_observation,
                )
                .env("CARGO_MANIFEST_DIR", fixture())
                .env("CARGO_PKG_NAME", PACKAGE)
                .env("CARGO_PKG_VERSION", "0.1.0")
                .env("CARGO_CRATE_NAME", CRATE_NAME),
            &output,
            &format!("{feature}.capture"),
            None,
        );
        timely(child_started.elapsed(), 300).unwrap();
        let text = std::str::from_utf8(&stdout).unwrap();
        let mut frames = text.lines().filter_map(|l| l.strip_prefix(PREFIX));
        let frame = frames.next().unwrap();
        assert!(frame.len() <= FRAME_CAP);
        assert!(frames.next().is_none());
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let observation: Value = serde_json::from_str(frame).unwrap();
        assert_eq!(
            observation["schema"],
            "fe2o3-test-source-lds-capture-observation-v22"
        );
        assert_eq!(
            observation["inputs"],
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(observation["actual_rustc_callback"], true);
        assert_eq!(
            observation["observation"]["cases"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(observation["observation"]["negative_capture_cases"], 4);
        assert_eq!(observation["observation"]["generic_debug_records"], 0);
        assert_eq!(inputs(&source, feature), expected);
        timely(child_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 650).unwrap();
        observations.push(observation);
    }
    assert_eq!(observations.len(), 2);
    require_current_source();
    let report=serde_json::to_vec_pretty(&json!({
        "schema":"fe2o3-test-source-lds-capture-ladder-v22","observations":observations,
        "actual_source_sessions":2,"positive_capture_cases":4,"negative_capture_cases":8,
        "generic_debug_refusals":2,"public_v22_cli":"unavailable","source_custody_from_bytes":false,
        "native_execution":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false
    })).unwrap();
    assert!(report.len() <= 128 * 1024);
    timely(started.elapsed(), 650).unwrap();
    fs::write(output.join("observation.json"), report).unwrap();
    timely(started.elapsed(), 650).unwrap();
}
#[test]
fn capture_feature_and_input_shape_controls_fail_closed() {
    for feature in [
        FEATURES[2],
        "physical-lds-exchange-one-v22,physical-lds-exchange-registers-v22",
        "../other",
        "",
    ] {
        assert!(positive_feature(feature).is_err());
    }
    for feature in &FEATURES[..2] {
        assert!(positive_feature(feature).is_ok());
    }
    assert!(serde_json::from_str::<Inputs>("{}").is_err());
}
#[test]
fn capture_final_observation_cannot_accept_at_or_after_deadline() {
    use std::time::Duration;
    for seconds in [300, 650] {
        assert!(
            timely(
                Duration::from_secs(seconds) - Duration::from_nanos(1),
                seconds
            )
            .is_ok()
        );
        assert!(timely(Duration::from_secs(seconds), seconds).is_err());
        assert!(
            timely(
                Duration::from_secs(seconds) + Duration::from_nanos(1),
                seconds
            )
            .is_err()
        );
    }
}

#[test]
fn inert_export_metadata_controls_reject_duplicate_foreign_or_changed_inputs() {
    // These are explicitly inert metadata fixtures, not canonical bytes or source owners.
    let record = PreparedInvocation {
        schema: "test-only-inert-invocation".into(),
        feature: FEATURES[0].into(),
        args: vec![],
        crate_binding: "inert".into(),
        cargo_observation: "inert".into(),
        source_sha256: "inert-source".into(),
        root_source_sha256: "inert-root".into(),
        manifest_sha256: "inert-manifest".into(),
        artifacts_sha256: "inert-artifacts".into(),
        metadata_sha256: "inert-metadata".into(),
    };
    let canonical = b"inert byte relation only";
    let llvm = b"inert text relation only";
    let row = json!({
        "schema":"fe2o3-test-source-physical-lds-exchange-observation-v22","feature":FEATURES[0],"mode":"diagnostic",
        "invocation":serde_json::to_value(&record).unwrap(),"actual_rustc_callback":true,"source_unchanged":true,
        "protected_finalizer_admitted":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false,
        "observation":{"stage":"public_pre_ranked_diagnostic_driver","canonical_sha256":digest(canonical),"llvm_sha256":digest(llvm)}
    });
    let mut rows = vec![Value::Null; 38];
    rows[0] = row.clone();
    let original =
        json!({"schema":"fe2o3-test-source-physical-lds-exchange-ladder-v22","observations":rows});
    assert!(export_join(&original, FEATURES[0], &record, canonical, llvm).is_ok());
    for mode in 0..6 {
        let mut changed = original.clone();
        match mode {
            0 => changed["schema"] = json!("wrong"),
            1 => {
                changed["observations"].as_array_mut().unwrap().pop();
            }
            2 => changed["observations"][1] = row.clone(),
            3 => changed["observations"][0]["invocation"]["source_sha256"] = json!("foreign"),
            4 => changed["observations"][0]["observation"]["canonical_sha256"] = json!("changed"),
            5 => changed["observations"][0]["grants_artifact_or_launch_authority"] = json!(true),
            _ => unreachable!(),
        }
        assert!(export_join(&changed, FEATURES[0], &record, canonical, llvm).is_err());
    }
    assert!(export_join(&original, FEATURES[1], &record, canonical, llvm).is_err());
    assert!(export_join(&original, FEATURES[0], &record, b"changed", llvm).is_err());
    assert!(export_join(&original, FEATURES[0], &record, canonical, b"changed").is_err());
}
