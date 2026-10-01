//! Private operational adapter. Root owns fresh Cargo work, process capture,
//! source/dependency closure pinning, deadlines, all nine rustc sessions and
//! qualification. This module never spawns a child or grants source authority.
use super::super::gfx942_inline_value_qualification_v30_tests::invocation_for_fixture_source_with_dependencies;
use super::{RetainedBytes, digest};
use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const ROOT_ENV: &str = "FE2O3_ORDERED_STAGE_OPERATIONAL_ROOT_V1";
const CASE_ENV: &str = "FE2O3_ORDERED_STAGE_BASELINE_CASE_V1";
const PACKAGE: &str = "fe2o3-production-extraction-fixture";
const CRATE_NAME: &str = "fe2o3_production_extraction_fixture";
const FILE_CAP: usize = 16 * 1024 * 1024;
const BASELINE_CAP: usize = 256 * 1024;
const CASES: [&str; 4] = ["one", "three", "sixteen", "invalid-count"];

fn feature(name: &str) -> Result<&'static str, &'static str> {
    match name {
        "one" => Ok("ordered-program-one-v32"),
        "three" => Ok("ordered-program-v32"),
        "sixteen" => Ok("ordered-program-sixteen-v32"),
        "invalid-count" => Ok("ordered-program-invalid-count-v32"),
        _ => Err("unknown or combined stage case"),
    }
}
fn positive(name: &str) -> bool {
    matches!(name, "one" | "three" | "sixteen")
}
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
fn root() -> PathBuf {
    let path = PathBuf::from(std::env::var_os(ROOT_ENV).expect("root-selected fresh preparation"));
    assert!(path.is_absolute());
    assert_eq!(path.canonicalize().unwrap(), path);
    assert!(fs::symlink_metadata(&path).unwrap().is_dir());
    path
}
fn write_new(directory: &Path, name: &str, value: &impl Serialize) {
    let bytes = serde_json::to_vec_pretty(value).unwrap();
    super::super::publish_new_inert_output(
        &directory.join(name),
        &bytes,
        128 * 1024,
        "ordered-stage preparation",
    )
    .unwrap();
}
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Pin {
    path: String,
    bytes: usize,
    sha256: String,
}
fn pin(label: &str, retained: &RetainedBytes) -> Pin {
    Pin {
        path: label.into(),
        bytes: retained.bytes.len(),
        sha256: digest(&retained.bytes),
    }
}
fn sources() -> Vec<Pin> {
    let mut result = Vec::new();
    for (relative, embedded) in [
        (
            "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs",
            include_bytes!("../../tests/fixtures/production-extraction-device/src/lib.rs")
                .as_slice(),
        ),
        (
            "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/ordered_program_v32.rs",
            include_bytes!(
                "../../tests/fixtures/production-extraction-device/src/ordered_program_v32.rs"
            )
            .as_slice(),
        ),
        (
            "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/Cargo.toml",
            include_bytes!("../../tests/fixtures/production-extraction-device/Cargo.toml")
                .as_slice(),
        ),
        (
            "crates/fe2o3-device/src/diagnostics.rs",
            include_bytes!("../../../fe2o3-device/src/diagnostics.rs").as_slice(),
        ),
        (
            "crates/fe2o3-device/src/ordered_program.rs",
            include_bytes!("../../../fe2o3-device/src/ordered_program.rs").as_slice(),
        ),
        (
            "crates/fe2o3-device/src/lib.rs",
            include_bytes!("../../../fe2o3-device/src/lib.rs").as_slice(),
        ),
    ] {
        let mut retained = RetainedBytes::open(&repository().join(relative), 1024 * 1024).unwrap();
        assert_eq!(
            retained.bytes, embedded,
            "rebuild current adapter after {relative} changes"
        );
        result.push(pin(relative, &retained));
        retained.recheck().unwrap();
    }
    result
}
fn check_feature(metadata: &Value, selected: &str) -> Result<(), &'static str> {
    if !CASES.iter().any(|name| feature(name) == Ok(selected)) {
        return Err("unlisted actual feature");
    }
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("packages")?;
    let mut matching = packages
        .iter()
        .filter(|p| p.get("name").and_then(Value::as_str) == Some(PACKAGE));
    let package = matching.next().ok_or("missing fixture package")?;
    if matching.next().is_some() {
        return Err("ambiguous fixture package");
    }
    if package
        .get("features")
        .and_then(Value::as_object)
        .and_then(|f| f.get(selected))
        .and_then(Value::as_array)
        .is_none_or(|f| !f.is_empty())
    {
        return Err("not an actual empty Cargo feature");
    }
    Ok(())
}
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Prepared {
    schema: String,
    case: String,
    feature: String,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
    source_files: Vec<Pin>,
    preparation_files: Vec<Pin>,
}
fn derive(directory: &Path, name: &str) -> Prepared {
    let selected = feature(name).unwrap();
    let mut sysroot = RetainedBytes::open(&directory.join("sysroot.stdout"), 4096).unwrap();
    let mut metadata = RetainedBytes::open(&directory.join("metadata.stdout"), FILE_CAP).unwrap();
    let mut artifacts =
        RetainedBytes::open(&directory.join("dependencies.stdout"), FILE_CAP).unwrap();
    check_feature(&serde_json::from_slice(&metadata.bytes).unwrap(), selected).unwrap();
    let before = sources();
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source_with_dependencies(
        (directory, directory),
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(selected),
        &fixture().join("src/lib.rs"),
        "gfx942",
    );
    // Existing helper derives fresh portable metadata from these actual paths,
    // package manifest, selected feature and exact Cargo artifact records.
    // These values are diagnostic bindings, not an imported invocation authority.
    assert_eq!(sources(), before);
    sysroot.recheck().unwrap();
    metadata.recheck().unwrap();
    artifacts.recheck().unwrap();
    Prepared {
        schema: "fe2o3-ordered-stage-prepared-invocation-v1".into(),
        case: name.into(),
        feature: selected.into(),
        args,
        crate_binding,
        cargo_observation,
        source_files: before,
        preparation_files: vec![
            pin("sysroot.stdout", &sysroot),
            pin("metadata.stdout", &metadata),
            pin("dependencies.stdout", &artifacts),
        ],
    }
}
fn checked_record(directory: &Path, name: &str) -> Prepared {
    let actual = derive(directory, name);
    let mut retained = RetainedBytes::open(
        &directory.join(format!("{name}.invocation.json")),
        128 * 1024,
    )
    .unwrap();
    let expected: Prepared = serde_json::from_slice(&retained.bytes).unwrap();
    assert_eq!(actual, expected, "preparation/source/invocation changed");
    retained.recheck().unwrap();
    actual
}
fn verify_environment(record: &Prepared) {
    for (key, expected) in [
        (CRATE_BINDING_ID_ENV_V1, record.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            record.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", PACKAGE),
        ("CARGO_PKG_VERSION", "0.1.0"),
        ("CARGO_CRATE_NAME", CRATE_NAME),
    ] {
        assert_eq!(std::env::var(key).unwrap(), expected);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        fixture()
    );
}

#[test]
#[ignore = "root prepares one fresh dependency directory and exact current Cargo/sysroot outputs"]
fn prepare_invocations() {
    let directory = root();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let mut records = Vec::new();
    for name in CASES {
        let record = derive(&directory, name);
        write_new(&directory, &format!("{name}.invocation.json"), &record);
        records.push(record);
    }
    write_new(
        &directory,
        "prepared.json",
        &json!({
            "schema":"fe2o3-ordered-stage-preparation-v1", "records":&records,
            "source_authentication_exported":false, "artifact_or_launch_authority":false,
            "dependency_closure_owner":"root independently pins and rechecks the fresh whole tree",
            "requires_root_provenance_for_preparation_commands":true,
            "compiler_sessions_in_this_preparation":0
        }),
    );
}

#[test]
#[ignore = "one root-supervised independent unchanged-route V17 baseline export per positive case"]
fn export_original_baseline() {
    let directory = root();
    let name = std::env::var(CASE_ENV).expect("exact positive baseline case");
    assert!(positive(&name), "malformed case is never a baseline");
    let before = checked_record(&directory, &name);
    verify_environment(&before);
    let output = directory.join(format!("{name}.baseline-v17.bin"));
    assert!(!output.exists(), "no baseline replacement");
    // Exactly the pre-existing source exporter, not the stage observer or an
    // inert model constructor. This performs one separate live rustc session.
    super::super::run_diagnostic_ordered_program_kir_extraction_driver_v17(&before.args, &output)
        .expect("unchanged original source baseline export");
    let mut baseline = RetainedBytes::open(&output, BASELINE_CAP).unwrap();
    let baseline_pin = pin(&format!("{name}.baseline-v17.bin"), &baseline);
    assert_eq!(checked_record(&directory, &name), before);
    verify_environment(&before);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    baseline.recheck().unwrap();
    let receipt = json!({
        "schema":"fe2o3-ordered-stage-original-baseline-v1",
        "case":&name, "feature":&before.feature, "invocation":&before,
        "baseline":baseline_pin, "actual_rustc_sessions":1,
        "original_unchanged_source_export_route":true,
        "source_authentication_exported":false, "artifact_or_launch_authority":false
    });
    write_new(&directory, &format!("{name}.baseline.json"), &receipt);
    println!(
        "\nFE2O3_ORDERED_STAGE_ORIGINAL_BASELINE_V1 {}",
        serde_json::to_string(&receipt).unwrap()
    );
}

#[test]
fn operational_case_names_are_closed_and_malformed_cannot_export() {
    for name in CASES {
        assert!(feature(name).is_ok());
    }
    for name in ["", "../one", "one,three", "ordered-program-one-v32", "17"] {
        assert!(feature(name).is_err());
        assert!(!positive(name));
    }
    assert!(!positive("invalid-count"));
    assert!(positive("one") && positive("three") && positive("sixteen"));
}
#[test]
fn operational_feature_checks_actual_unique_empty_cargo_feature() {
    let key = feature("one").unwrap();
    let good = json!({"packages":[{"name":PACKAGE, "features":{key:[]}}]});
    assert!(check_feature(&good, key).is_ok());
    for bad in [
        json!({"packages":[{"name":PACKAGE, "features":{}, "lib":{key:[]}}]}),
        json!({"packages":[{"name":PACKAGE, "features":{key:["other"]}}]}),
        json!({"packages":[{"name":"other", "features":{key:[]}}]}),
        json!({"packages":[{"name":PACKAGE,"features":{key:[]}},{"name":PACKAGE,"features":{key:[]}}]}),
    ] {
        assert!(check_feature(&bad, key).is_err());
    }
    assert!(check_feature(&good, "ordered-program-unlisted-v32").is_err());
}
#[test]
fn operational_prepared_record_rejects_incomplete_or_extra_fields() {
    assert!(serde_json::from_str::<Prepared>("{}").is_err());
    let record = json!({
        "schema":"fe2o3-ordered-stage-prepared-invocation-v1","case":"one",
        "feature":"ordered-program-one-v32","args":["inert"],"crate_binding":"inert",
        "cargo_observation":"inert","source_files":[],"preparation_files":[]
    });
    assert!(serde_json::from_value::<Prepared>(record.clone()).is_ok());
    let mut extra = record;
    extra
        .as_object_mut()
        .unwrap()
        .insert("authority".into(), json!(true));
    assert!(serde_json::from_value::<Prepared>(extra).is_err());
}
