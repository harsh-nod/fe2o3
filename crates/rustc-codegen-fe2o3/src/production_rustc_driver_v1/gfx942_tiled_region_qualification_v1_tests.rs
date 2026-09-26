//! Actual-source P0 qualification, not normal ranked/formal/target acceptance.
//! The first four-session gate measures the finite source/ledger seam; the
//! separate shape gate must not pass merely because an alias is spelled Rust.
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
#[path = "gfx942_tiled_region_inputs_v1_tests.rs"]
mod inputs;
#[path = "gfx942_tiled_region_observation_v1_tests.rs"]
pub(super) mod observation;

const OUTPUT_ENV: &str = "FE2O3_TEST_TILED_REGION_OUTPUT_V1";
const CHILD_ENV: &str = "FE2O3_TEST_TILED_REGION_INPUTS_V1";
const CASE_ENV: &str = "FE2O3_TEST_TILED_REGION_CASE_V1";
const PACKAGE: &str = "fe2o3-tiled-region-inspection-v1-fixture";
const CRATE_NAME: &str = "fe2o3_tiled_region_inspection_v1_fixture";
const CHILD: &str = "production_rustc_driver_v1::gfx942_tiled_region_qualification_v1_tests::actual_bf16_source_child";
const PREFIX: &str = "FE2O3_TILED_REGION_SOURCE_OBSERVATION_V1 ";
const FEATURES: [&str; 6] = [
    "direct",
    "single-predecessor",
    "phi",
    "loop-carried",
    "retained-memory",
    "wrong-launch",
];
const CORE: [&str; 4] = ["direct", "wrong-launch", "direct-error", "direct-panic"];
const SHAPES: [&str; 4] = [
    "single-predecessor",
    "phi",
    "loop-carried",
    "retained-memory",
];
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tiled-region-inspection-v1")
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
        "direct-error" | "direct-panic" => Ok("direct"),
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
    normal: bool,
    directory: &'a Path,
    started: std::time::Instant,
}
impl Callbacks for BodyCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(
            match super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            ) {
                Ok(transaction) => {
                    if self.normal {
                        observation::normal::observe(transaction, self.directory, self.started)
                    } else {
                        observation::observe(transaction, self.case).unwrap()
                    }
                }
                Err(error) => {
                    json!({"stage":"actual_source_or_callback_refused","diagnostic":error,
                "phase":Value::Null,"callback":Value::Null,"collection_refused":true})
                }
            },
        );
        Compilation::Stop
    }
}
#[test]
#[ignore = "actual pinned-rustc child; use the isolated source core or shape ladder"]
fn actual_bf16_source_child() {
    source_child(false);
}
fn source_child(normal: bool) {
    let started = std::time::Instant::now();
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("actual preparation directory"));
    assert!(directory.is_absolute());
    let case = std::env::var(CASE_ENV).expect("closed actual source case");
    if normal {
        assert!(matches!(case.as_str(), "direct" | "wrong-launch"));
    }
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
        "actual source/metadata/dependency/invocation identity drift"
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
        normal,
        directory: &directory,
        started,
    };
    timely(started.elapsed(), 300).unwrap();
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "one genuine active rustc callback");
    let observed = callbacks.result.expect("actual callback absent");
    // Retain the exact observed refusal/shape BEFORE the oracle. A first-run
    // cap/span/boundary mismatch is a failed gate, never a guessed success.
    publish_json(
        &directory,
        &format!("{case}.observed.json"),
        &json!({
        "case":case,"observation":observed,"accepted":false,"acceptance_requires_completed_parent":true}),
    );
    if normal {
        observation::normal::recheck_outputs(&directory, &case, &observed);
    } else {
        observation::accept(&case, &observed).unwrap();
    }
    let source_observation = if normal {
        &observed["inspection"]
    } else {
        &observed
    };
    if let Some(bytes) = source_observation["snapshot"]["source_sha256"].as_array() {
        let actual_bytes = read_bounded(&fixture().join("src/lib.rs"), 65536).unwrap();
        assert_eq!(
            Value::Array(bytes.clone()),
            serde_json::to_value(<[u8; 32]>::from(Sha256::digest(&actual_bytes))).unwrap()
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
    let frame = serde_json::to_string(&json!({
        "schema":if normal { observation::normal::SCHEMA } else { "fe2o3-test-tiled-region-source-observation-v1" },"case":case,"feature":feature,
        "invocation":actual,"observation":observed,"actual_rustc_callbacks":callbacks.calls,
        "source_and_dependencies_unchanged":true,"pre_ranked_only":!normal,
        "normal_ranked_formal_target_handoff_qualified":normal && case=="direct","numerical_cpu_qualified":false,
        "grants_artifact_or_launch_authority":false,"hardware_observed":false,
    }))
    .unwrap();
    assert!(frame.len() <= 128 * 1024);
    timely(started.elapsed(), 300).unwrap();
    let prefix = if normal {
        observation::normal::PREFIX
    } else {
        PREFIX
    };
    println!("\n{prefix}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
fn ladder(cases: &[&str], kind: &str) {
    let normal = kind == observation::normal::GATE;
    assert!(
        normal
            || matches!(
                kind,
                "core-four-sessions" | "shape-four-sessions-pending-measurement"
            )
    );
    if normal {
        assert_eq!(cases, ["direct", "wrong-launch"]);
    }
    let child_test = if normal {
        observation::normal::CHILD
    } else {
        CHILD
    };
    let prefix = if normal {
        observation::normal::PREFIX
    } else {
        PREFIX
    };
    let schema = if normal {
        observation::normal::SCHEMA
    } else {
        "fe2o3-test-tiled-region-source-observation-v1"
    };
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
    checked(sanitized(&mut cargo).current_dir(repository()).args([
        "check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device",
        "--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path",
    ]).arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&dependency_target)
        .env("RUSTC",sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS","-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &directory,"dependencies",Some(&dependency_target));
    timely(started.elapsed(), 1200).unwrap();
    fs::create_dir(directory.join("analysis-output")).unwrap();
    let (dependencies, files) = inputs::dependency_snapshot(&directory);
    publish_json(&directory, "dependency-files.json", &files);
    drop(files);
    assert_eq!(inputs::current_sources(), sources);
    let mut observations = Vec::new();
    for case in cases {
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
                .args(["--exact", child_test, "--ignored", "--nocapture"])
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
            .filter_map(|line| line.strip_prefix(prefix))
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
        assert_eq!(observed["schema"], schema);
        assert_eq!(observed["case"], *case);
        assert_eq!(observed["feature"], feature);
        assert_eq!(
            observed["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observed["actual_rustc_callbacks"], 1);
        assert_eq!(observed["source_and_dependencies_unchanged"], true);
        if normal {
            observation::normal::recheck_outputs(&directory, case, &observed["observation"]);
            assert_eq!(observed["pre_ranked_only"], false);
            assert_eq!(
                observed["normal_ranked_formal_target_handoff_qualified"],
                *case == "direct"
            );
            assert_eq!(observed["numerical_cpu_qualified"], false);
            assert_eq!(observed["grants_artifact_or_launch_authority"], false);
            assert_eq!(observed["hardware_observed"], false);
        } else {
            observation::accept(case, &observed["observation"]).unwrap();
        }
        let raw: Value = serde_json::from_slice(
            &read_bounded(&directory.join(format!("{case}.observed.json")), 256 * 1024).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["case"], *case);
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
    assert_eq!(observations.len(), cases.len());
    assert_eq!(inputs::current_sources(), sources);
    assert_eq!(inputs::dependency_snapshot(&directory).0, dependencies);
    if normal {
        // Rehash earlier positive bytes after the refused child and every
        // final input/dependency observation, before accepting the union.
        for row in &observations {
            observation::normal::recheck_outputs(
                &directory,
                row["case"].as_str().unwrap(),
                &row["observation"],
            );
        }
    }
    let report = json!({"schema":if normal { "fe2o3-test-tiled-region-normal-ladder-v1" } else { "fe2o3-test-tiled-region-source-ladder-v1" },"gate":kind,
        "actual_rustc_sessions":observations.len(),"observations":observations,"source_files":sources,
        "dependency_snapshot":dependencies,"fresh_dependency_builds":1,"pre_ranked_only":!normal,
        "all_required_shape_controls_complete":false,"numerical_cpu_qualified":false,
        "normal_ranked_formal_target_handoff_qualified":normal,"source_publication_attempted":false,
        "native_execution_attempted":false,"grants_artifact_or_launch_authority":false,
        "cleanup_scope":"reused bounded direct-child/process-group helper, not whole-family supervision",
        "acceptance":"completed successful parent test required; this JSON is historical observation only"});
    timely(started.elapsed(), 1200).unwrap();
    publish_json(&directory, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
#[ignore = "first genuine finite P0 gate: direct source, launch refusal, actual callback error and panic"]
fn actual_bf16_source_core_ladder() {
    ladder(&CORE, "core-four-sessions");
}
#[test]
#[ignore = "separate actual-shape controls: may refuse if optimization erased intended alias/memory shape"]
fn actual_bf16_source_shape_controls() {
    ladder(&SHAPES, "shape-four-sessions-pending-measurement");
}
#[test]
fn bf16_source_controls_keep_closed_case_names_and_original_deadlines() {
    for case in ["../direct", "direct,phi", "direct-error,wrong-launch", ""] {
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
        intended_output(Path::new("/task"), std::ffi::OsStr::new("fresh-p0"), repo).unwrap(),
        Path::new("/task/fresh-p0")
    );
}

#[test]
fn bf16_source_fixture_is_no_std_and_partitions_the_loop_before_macro_expansion() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/tiled-region-inspection-v1/src/lib.rs"
    ));
    assert!(source.contains("\n#![no_std]\n"));
    let (direct, looping) = source
        .split_once("#[cfg(feature = \"loop-carried\")]\n#[kernel(")
        .expect("loop control has its own cfg-gated kernel item");
    assert!(direct.contains("#[cfg(not(feature = \"loop-carried\"))]"));
    assert!(!direct.contains("while left"));
    assert!(!direct.contains("control_flow(loop_bounds("));
    assert_eq!(
        direct.matches("pub fn tiled_region_inspection_v1(").count(),
        1
    );
    assert_eq!(
        looping
            .matches("pub fn tiled_region_inspection_v1(")
            .count(),
        1
    );
    assert!(looping.contains("control_flow(loop_bounds(3))"));
    assert!(looping.contains("let mut left = selector & 3;"));
    assert!(looping.contains("while left != 0"));
}
