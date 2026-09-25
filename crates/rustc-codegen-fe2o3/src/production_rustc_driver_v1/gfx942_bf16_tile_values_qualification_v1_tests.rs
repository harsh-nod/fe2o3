//! Five genuine compiler sessions for authenticated helper transport only.
//! Successful transport MUST still encounter the unchanged ordinary refusal.
use super::gfx942_inline_value_qualification_v30_tests::{
    checked, invocation_for_fixture, read_bounded, sanitized,
};
use super::{Callbacks, Compilation, Compiler, TyCtxt};
use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[path = "gfx942_bf16_tile_values_inputs_v1_tests.rs"]
mod inputs;
#[path = "gfx942_bf16_tile_values_observation_v1_tests.rs"]
mod observation;

const OUTPUT_ENV: &str = "FE2O3_TEST_BF16_TILE_VALUES_OUTPUT_V1";
const CHILD_ENV: &str = "FE2O3_TEST_BF16_TILE_VALUES_INPUTS_V1";
const CASE_ENV: &str = "FE2O3_TEST_BF16_TILE_VALUES_CASE_V1";
const PACKAGE: &str = "fe2o3-bf16-tile-promotion-v1-fixture";
const CRATE_NAME: &str = "fe2o3_bf16_tile_promotion_v1_fixture";
const CHILD: &str = "production_rustc_driver_v1::gfx942_bf16_tile_values_qualification_v1_tests::actual_bf16_tile_values_child";
const PREFIX: &str = "FE2O3_BF16_TILE_VALUES_SOURCE_OBSERVATION_V1 ";
const FEATURES: [&str; 3] = ["identity", "swap01", "wrong-launch"];
const CORE: [&str; 5] = [
    "identity",
    "swap01",
    "wrong-launch",
    "identity-error",
    "identity-panic",
];
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bf16-tile-promotion-v1")
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
        "BF16 source qualification JSON",
    )
    .unwrap();
}
fn checked_feature(feature: &str) -> Result<(), &'static str> {
    FEATURES
        .contains(&feature)
        .then_some(())
        .ok_or("unknown or combined BF16 source feature")
}
fn feature_for_case(case: &str) -> Result<&str, &'static str> {
    match case {
        "identity-error" | "identity-panic" => Ok("identity"),
        other => {
            checked_feature(other)?;
            Ok(other)
        }
    }
}
fn timely(elapsed: std::time::Duration, seconds: u64) -> Result<(), &'static str> {
    (elapsed < std::time::Duration::from_secs(seconds))
        .then_some(())
        .ok_or("original monotonic source qualification deadline")
}
fn intended_output(
    parent: &Path,
    name: &std::ffi::OsStr,
    repository: &Path,
) -> Result<PathBuf, &'static str> {
    let name = Path::new(name);
    if name.components().count() != 1
        || !matches!(
            name.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err("fresh output final component");
    }
    let intended = parent.join(name);
    if !parent.is_absolute()
        || intended.starts_with(repository)
        || repository.starts_with(&intended)
    {
        return Err("output overlaps compiler candidate");
    }
    Ok(intended)
}
fn create_output(requested: &Path) -> PathBuf {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    assert!(requested.is_absolute());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    let intended = intended_output(&parent, requested.file_name().unwrap(), &repository()).unwrap();
    assert!(
        !intended.try_exists().unwrap(),
        "create-new task output only"
    );
    let handle = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(&parent)
        .unwrap();
    let before = handle.metadata().unwrap();
    assert!(before.is_dir());
    fs::create_dir(&intended).unwrap();
    let named = fs::metadata(&parent).unwrap();
    assert_eq!((before.dev(), before.ino()), (named.dev(), named.ino()));
    assert_eq!(intended.canonicalize().unwrap(), intended);
    assert!(
        !fs::symlink_metadata(&intended)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    intended
}
struct BodyCallbacks<'a> {
    case: &'a str,
    calls: usize,
    result: Option<Value>,
}
impl Callbacks for BodyCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(
            match super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            ) {
                Ok(transaction) => observation::observe(transaction, self.case),
                Err(error) => {
                    json!({"stage":"actual_source_or_callback_refused","diagnostic":error,"phase":null,"snapshot":null,"callback":null,"collection_refused":true})
                }
            },
        );
        Compilation::Stop
    }
}
#[test]
#[ignore = "genuine isolated helper source child; invoke through the five-session parent"]
fn actual_bf16_tile_values_child() {
    let started = std::time::Instant::now();
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("actual preparation directory"));
    assert!(directory.is_absolute());
    let case = std::env::var(CASE_ENV).expect("closed source case");
    let feature = feature_for_case(&case).unwrap();
    let actual = inputs::derive_record(&directory, feature);
    let retained: inputs::PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{case}.invocation.json")),
            128 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, retained,
        "actual source/metadata/dependency/invocation drift"
    );
    for (key, value) in [
        (CRATE_BINDING_ID_ENV_V1, actual.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            actual.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", PACKAGE),
        ("CARGO_PKG_VERSION", "0.0.0"),
        ("CARGO_CRATE_NAME", CRATE_NAME),
    ] {
        assert_eq!(std::env::var(key).unwrap(), value);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        fixture()
    );
    super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let mut callbacks = BodyCallbacks {
        case: &case,
        calls: 0,
        result: None,
    };
    timely(started.elapsed(), 300).unwrap();
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "one actual rustc callback");
    let observed = callbacks.result.expect("actual callback absent");
    // Failed first-run cardinality/ABI/span/SSA assumptions are retained, not
    // rewritten as the expected refusal and never counted as a positive.
    publish_json(
        &directory,
        &format!("{case}.observed.json"),
        &json!({"case":case,"observation":observed,"accepted":false,"acceptance_requires_completed_parent":true}),
    );
    observation::accept(&case, &observed).unwrap();
    if let Some(bytes) = observed["snapshot"]["source_sha256"].as_array() {
        assert_eq!(
            Value::Array(bytes.clone()),
            serde_json::to_value(<[u8; 32]>::from(Sha256::digest(
                read_bounded(&fixture().join("src/lib.rs"), 65536).unwrap()
            )))
            .unwrap()
        );
    }
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(inputs::derive_record(&directory, feature), actual);
    timely(started.elapsed(), 300).unwrap();
    let frame=serde_json::to_string(&json!({"schema":"fe2o3-test-bf16-tile-values-source-observation-v1","case":case,"feature":feature,"invocation":actual,"observation":observed,"actual_rustc_callbacks":callbacks.calls,"source_and_dependencies_unchanged":true,"transport_only":true,"emitted_helper_qualified":false,"normal_qualified":false,"numerical_cpu_qualified":false,"grants_artifact_or_launch_authority":false,"hardware_observed":false})).unwrap();
    assert!(frame.len() <= 128 * 1024);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
#[test]
#[ignore = "five genuine compiler sessions: Identity/Swap01 transport, launch refusal, callback error/panic"]
fn actual_bf16_tile_values_ladder() {
    let started = std::time::Instant::now();
    let requested = PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh task-owned output"));
    let directory = create_output(&requested);
    let sources = inputs::current_sources();
    let rustc_path =
        PathBuf::from(std::env::var_os("RUSTC").expect("absolute pinned-nightly RUSTC"));
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
    checked(sanitized(&mut cargo).current_dir(repository()).args(["check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device","--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path"]).arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&dependency_target).env("RUSTC",sysroot.join("bin/rustc")).env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS","-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),&directory,"dependencies",Some(&dependency_target));
    timely(started.elapsed(), 1200).unwrap();
    fs::create_dir(directory.join("analysis-output")).unwrap();
    let (dependencies, files) = inputs::dependency_snapshot(&directory);
    publish_json(&directory, "dependency-files.json", &files);
    drop(files);
    assert_eq!(inputs::current_sources(), sources);
    let mut observations = Vec::new();
    for case in CORE {
        let child_started = std::time::Instant::now();
        let feature = feature_for_case(case).unwrap();
        let record = inputs::derive_record(&directory, feature);
        assert_eq!(record.sources, sources);
        assert_eq!(record.dependencies, dependencies);
        publish_json(&directory, &format!("{case}.invocation.json"), &record);
        let mut child = Command::new(std::env::current_exe().unwrap());
        timely(started.elapsed(), 1200).unwrap();
        timely(child_started.elapsed(), 300).unwrap();
        let stdout = checked(
            sanitized(&mut child)
                .current_dir(repository())
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(CHILD_ENV, &directory)
                .env(CASE_ENV, case)
                .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
                .env(
                    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                    &record.cargo_observation,
                )
                .env("CARGO_MANIFEST_DIR", fixture())
                .env("CARGO_PKG_NAME", PACKAGE)
                .env("CARGO_PKG_VERSION", "0.0.0")
                .env("CARGO_CRATE_NAME", CRATE_NAME),
            &directory,
            case,
            None,
        );
        timely(child_started.elapsed(), 300).unwrap();
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
            "fe2o3-test-bf16-tile-values-source-observation-v1"
        );
        assert_eq!(observed["case"], case);
        assert_eq!(observed["feature"], feature);
        assert_eq!(
            observed["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observed["actual_rustc_callbacks"], 1);
        assert_eq!(observed["source_and_dependencies_unchanged"], true);
        assert_eq!(observed["transport_only"], true);
        for key in [
            "emitted_helper_qualified",
            "normal_qualified",
            "numerical_cpu_qualified",
            "grants_artifact_or_launch_authority",
            "hardware_observed",
        ] {
            assert_eq!(observed[key], false);
        }
        observation::accept(case, &observed["observation"]).unwrap();
        let raw: Value = serde_json::from_slice(
            &read_bounded(&directory.join(format!("{case}.observed.json")), 256 * 1024).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["case"], case);
        assert_eq!(raw["observation"], observed["observation"]);
        assert_eq!(raw["accepted"], false);
        assert_eq!(inputs::derive_record(&directory, feature), record);
        timely(child_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        publish_json(&directory, &format!("{case}.accepted.json"), &observed);
        timely(child_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        observations.push(observed);
    }
    assert_eq!(observations.len(), 5);
    assert_eq!(inputs::current_sources(), sources);
    assert_eq!(inputs::dependency_snapshot(&directory).0, dependencies);
    let report = json!({"schema":"fe2o3-test-bf16-tile-values-source-ladder-v1","actual_rustc_sessions":5,"expected_positive_transports":2,"expected_unchanged_normal_refusals":2,"expected_source_refusals":1,"expected_callback_error_panic":2,"observations":observations,"source_files":sources,"dependency_snapshot":dependencies,"fresh_dependency_builds":1,"transport_only":true,"emitted_helper_qualified":false,"normal_qualified":false,"numerical_cpu_qualified":false,"source_publication_attempted":false,"native_execution_attempted":false,"grants_artifact_or_launch_authority":false,"cleanup_scope":"reused bounded direct-child/process-group helper, not whole-family supervision","acceptance":"completed successful parent test required; JSON is historical only"});
    timely(started.elapsed(), 1200).unwrap();
    publish_json(&directory, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
fn bf16_source_controls_keep_closed_case_names_and_original_deadlines() {
    for case in [
        "../identity",
        "identity,swap01",
        "identity-error,wrong-launch",
        "",
    ] {
        assert!(feature_for_case(case).is_err());
    }
    for seconds in [300, 1200] {
        let exact = std::time::Duration::from_secs(seconds);
        assert!(timely(exact - std::time::Duration::from_nanos(1), seconds).is_ok());
        assert!(timely(exact, seconds).is_err());
        assert!(timely(exact + std::time::Duration::from_nanos(1), seconds).is_err());
    }
}
#[test]
fn bf16_source_output_location_refuses_candidate_before_create() {
    let repo = Path::new("/task/candidate");
    assert!(intended_output(repo, std::ffi::OsStr::new("outputs"), repo).is_err());
    assert!(intended_output(Path::new("/task"), std::ffi::OsStr::new("candidate"), repo).is_err());
    assert!(
        intended_output(
            Path::new("/task"),
            std::ffi::OsStr::new("../elsewhere"),
            repo
        )
        .is_err()
    );
    assert_eq!(
        intended_output(
            Path::new("/task"),
            std::ffi::OsStr::new("fresh-helper"),
            repo
        )
        .unwrap(),
        Path::new("/task/fresh-helper")
    );
}

#[test]
fn helper_fixture_is_separate_and_has_one_retained_nominal_source_call() {
    let source = include_str!("../../tests/fixtures/bf16-tile-promotion-v1/src/lib.rs");
    assert!(source.contains("#![no_std]"));
    assert_eq!(source.matches("#[inline(never)]").count(), 1);
    assert_eq!(source.matches("fn __fe2o3_bf16_tile").count(), 1);
    assert_eq!(source.matches("let result = __fe2o3_bf16_tile").count(), 1);
    assert_eq!(source.matches(".multiply_accumulate(").count(), 1);
    assert_eq!(source.matches(".into_values()").count(), 1);
    assert!(source.contains("Bf16MfmaAFragment<'wave>"));
    assert!(source.contains("F32AccumulatorFragment<'wave>"));
}
