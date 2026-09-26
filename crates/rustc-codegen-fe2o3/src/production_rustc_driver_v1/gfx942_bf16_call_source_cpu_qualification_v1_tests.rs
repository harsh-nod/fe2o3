//! Five separate genuine-source sessions for retained helper CPU observation.
//! This sibling never changes or satisfies the historical transport-only gate.
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
use super::gfx942_bf16_tile_values_qualification_v1_tests::observation as source_observation;
use super::gfx942_tiled_region_qualification_v1_tests::observation::cpu::oracle;
use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as Error;
#[path = "gfx942_bf16_call_source_cpu_capture_v1_tests.rs"]
mod capture;
#[path = "gfx942_bf16_call_source_cpu_observation_v1_tests.rs"]
mod observed;

const OUTPUT_ENV: &str = "FE2O3_TEST_BF16_CALL_CPU_OUTPUT_V1";
const CHILD_ENV: &str = "FE2O3_TEST_BF16_CALL_CPU_INPUTS_V1";
const CASE_ENV: &str = "FE2O3_TEST_BF16_CALL_CPU_CASE_V1";
const PACKAGE: &str = "fe2o3-bf16-tile-promotion-v1-fixture";
const CRATE_NAME: &str = "fe2o3_bf16_tile_promotion_v1_fixture";
const CHILD: &str = "production_rustc_driver_v1::gfx942_bf16_call_source_cpu_qualification_v1_tests::actual_bf16_call_source_cpu_child";
const PREFIX: &str = "FE2O3_BF16_CALL_SOURCE_CPU_OBSERVATION_V1 ";
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
                Ok(transaction) => observed::observe(transaction, self.case),
                Err(error) => {
                    json!({"stage":"actual_helper_source_cpu_refused","diagnostic":error,"phase":null,"cpu":null,"progress":null,"collection_refused":true})
                }
            },
        );
        Compilation::Stop
    }
}
#[test]
#[ignore = "genuine isolated helper source child; invoke through the five-session parent"]
fn actual_bf16_call_source_cpu_child() {
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
    accept(&case, &observed).unwrap();
    if let Some(bytes) = observed["cpu"]["source"]["source_sha256"].as_array() {
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
    let frame=serde_json::to_string(&json!({"schema":"fe2o3-test-bf16-call-source-cpu-observation-v1","case":case,"feature":feature,"invocation":actual,"observation":observed,"actual_rustc_callbacks":callbacks.calls,"source_and_dependencies_unchanged":true,"transport_only":false,"emitted_helper_qualified":matches!(case.as_str(),"identity"|"swap01"),"normal_qualified":false,"numerical_cpu_qualified":matches!(case.as_str(),"identity"|"swap01"),"grants_artifact_or_launch_authority":false,"hardware_observed":false})).unwrap();
    assert!(frame.len() <= 256 * 1024);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
#[test]
#[ignore = "five genuine compiler sessions: Identity/Swap01 transport, launch refusal, callback error/panic"]
fn actual_bf16_call_source_cpu_ladder() {
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
            "fe2o3-test-bf16-call-source-cpu-observation-v1"
        );
        assert_eq!(observed["case"], case);
        assert_eq!(observed["feature"], feature);
        assert_eq!(
            observed["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observed["actual_rustc_callbacks"], 1);
        assert_eq!(observed["source_and_dependencies_unchanged"], true);
        assert_eq!(observed["transport_only"], false);
        assert_eq!(
            observed["emitted_helper_qualified"],
            matches!(case, "identity" | "swap01")
        );
        assert_eq!(
            observed["numerical_cpu_qualified"],
            matches!(case, "identity" | "swap01")
        );
        for key in [
            "normal_qualified",
            "grants_artifact_or_launch_authority",
            "hardware_observed",
        ] {
            assert_eq!(observed[key], false);
        }
        accept(case, &observed["observation"]).unwrap();
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
    let report = json!({"schema":"fe2o3-test-bf16-call-source-cpu-ladder-v1","actual_rustc_sessions":5,"expected_positive_sources":2,"expected_positive_numerical_runs":36,"expected_request_refusals":32,"expected_unchanged_normal_refusals":2,"expected_source_refusals":1,"expected_callback_error_panic":2,"callback_controls_completed_numerical_runs":2,"observations":observations,"source_files":sources,"dependency_snapshot":dependencies,"fresh_dependency_builds":1,"transport_only":false,"emitted_helper_qualified":true,"normal_qualified":false,"numerical_cpu_qualified":true,"source_publication_attempted":false,"native_execution_attempted":false,"grants_artifact_or_launch_authority":false,"cleanup_scope":"reused bounded direct-child/process-group helper, not whole-family supervision","acceptance":"completed successful parent test required; JSON is historical only"});
    timely(started.elapsed(), 1200).unwrap();
    publish_json(&directory, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}

fn count(value: &Value) -> Result<u64, &'static str> {
    value.as_u64().ok_or("missing actual nonnegative counter")
}
fn accept(case: &str, report: &Value) -> Result<(), &'static str> {
    if !CORE.contains(&case) {
        return Err("unknown source CPU case");
    }
    let diagnostic = report["diagnostic"]
        .as_str()
        .ok_or("actual refusal diagnostic absent")?;
    if case == "wrong-launch" {
        if report["stage"] != "actual_helper_source_cpu_refused"
            || !report["phase"].is_null()
            || !report["cpu"].is_null()
            || !report["progress"].is_null()
        {
            return Err("wrong launch did not refuse before nominal callback");
        }
        if !diagnostic.contains("BF16 helper requires explicit WG64 and one workgroup") {
            return Err("wrong launch diagnostic differs");
        }
        return Ok(());
    }
    let control = matches!(case, "identity-error" | "identity-panic");
    let cpu = &report["cpu"];
    let phase = &report["phase"];
    let permutation = if case == "swap01" {
        [1, 0, 2, 3]
    } else {
        [0, 1, 2, 3]
    };
    let source = &cpu["source"];
    if report["stage"] != "actual_helper_source_cpu_observed"
        || report["unexpected_normal_success"] != false
        || report["normal_qualified"] != false
        || report["hardware_observed"] != false
        || report["source_authority_in_copied_row"] != false
        || phase["same_ledger"] != true
        || phase["failed_work"] != false
        || phase["failed_storage"] != false
        || phase["normal_succeeded"] != false
        || phase["materialized"] != !control
        || phase["normal_attempted"] != !control
        || count(&phase["occurrence_storage"])? == 0
        || count(&phase["nominal_storage"])? == 0
        || count(&phase["reverification_peak_storage"])? > 2 * 1024 * 1024 * 1024
        || count(&phase["phase_peak_storage"])? > 2 * 1024 * 1024 * 1024
        || count(&phase["phase_peak_storage"])? < count(&phase["reverification_peak_storage"])?
        || cpu["same_original_ledger"] != true
        || cpu["source_authority_in_copied_row"] != false
        || source["return_permutation"] != json!(permutation)
        || cpu["sites"]["permutation"] != json!(permutation)
        || source["root"] == source["helper"]
        || cpu["sites"]["root"] == cpu["sites"]["helper"]
        || source["source_authority_in_copied_row"] != false
        || count(&cpu["canonical_bytes"])? == 0
        || count(&cpu["canonical_bytes"])? > 16384
    {
        return Err("actual source, nominal owner, original ledger or two-envelope peak differs");
    }
    for field in [
        "source_sha256",
        "root_mir_sha256",
        "helper_mir_sha256",
        "semantic_sha256",
        "helper_source_signature_sha256",
        "helper_fn_abi_sha256",
    ] {
        if source[field].as_array().is_none_or(|a| a.len() != 32) {
            return Err("actual Rust/MIR/FnABI identity absent");
        }
    }
    if source["helper_actual_abi_modes"] != json!([1, 3, 3, 4, 4]) {
        return Err("actual Rust helper FnABI modes differ");
    }
    let accounting = report["accounting"]
        .as_array()
        .ok_or("actual callback accounting absent")?;
    if accounting.len() != 6 {
        return Err("callback accounting shape");
    }
    let reserve = count(&accounting[4])?;
    let extra = if control { 23 } else { 0 };
    if reserve == 0
        || reserve > 131072
        || count(&accounting[5])? != extra
        || count(&accounting[1])?
            != count(&accounting[0])?
                .checked_add(reserve + extra)
                .ok_or("callback overflow")?
        || count(&accounting[3])? <= count(&accounting[2])?
        || count(&phase["work"])? < count(&accounting[3])?
    {
        return Err("actual CPU callback floor/work differ");
    }
    let retained = if control {
        0
    } else {
        count(&phase["occurrence_storage"])?
            .checked_add(count(&phase["nominal_storage"])?)
            .ok_or("retained overflow")?
    };
    if count(&phase["final_storage"])?
        != count(&phase["entry_storage"])?
            .checked_add(reserve + extra)
            .and_then(|v| v.checked_add(retained))
            .ok_or("phase overflow")?
    {
        return Err("source/SSA owner drop-before-refund or retained receipt differs");
    }
    if control {
        let marker = if case == "identity-panic" {
            "CallbackPanicked"
        } else {
            "genuine helper CPU source callback error control"
        };
        if !diagnostic.contains(marker) {
            return Err("source CPU callback control not reached");
        }
    } else if !diagnostic.contains("BF16 nominal source-ranked projection") {
        return Err("ordinary nominal consumer refusal not reached");
    }
    let expected_attempts = if control { 1 } else { 34 };
    if count(&cpu["attempted_runs"])? != expected_attempts
        || report["progress"]["floor_restored"] != true
        || !report["progress"]["debug_failure"].is_null()
    {
        return Err("numerical run series incomplete");
    }
    let runs = cpu["runs"]
        .as_array()
        .ok_or("actual numerical runs absent")?;
    let negatives = cpu["negatives"]
        .as_array()
        .ok_or("actual negative controls absent")?;
    if runs.len() != 18 || negatives.len() != 16 {
        return Err("finite numerical row counts differ");
    }
    for (index, run) in runs.iter().enumerate() {
        if control && index > 0 {
            if !run.is_null() {
                return Err("callback control continued numerical runs");
            }
            continue;
        }
        let pattern = index / 3;
        let length = oracle::LENGTHS[index % 3];
        let helper = oracle::expected(pattern);
        let caller = capture::expected_call(&helper, permutation);
        let stores = if length == 64 {
            u64::MAX
        } else {
            (1u64 << length) - 1
        };
        let backing = run["actual_allocations"]
            .as_array()
            .ok_or("backing identity absent")?;
        if run["pattern"] != pattern
            || run["output_length"] != length
            || run["helper_values_row_major_le_hex"] != serde_json::to_value(helper).unwrap()
            || run["caller_values_row_major_le_hex"] != serde_json::to_value(caller).unwrap()
            || run["output_with_canaries_le_hex"]
                != serde_json::to_value(capture::expected_output(pattern, length, permutation))
                    .unwrap()
            || count(&run["helper_lane_mask"])? != u64::MAX
            || count(&run["caller_lane_mask"])? != u64::MAX
            || count(&run["committed_store_lane_mask"])? != stores
            || count(&run["records"])? == 0
            || count(&run["steps"])? == 0
            || count(&run["storage_floor"])? != count(&run["storage_after"])?
            || count(&run["work_after"])? <= count(&run["work_before"])?
            || backing.len() != 3
            || backing[0] == backing[1]
            || backing[0] == backing[2]
            || backing[1] == backing[2]
        {
            return Err("actual four-component helper/caller/frame/output observation differs");
        }
    }
    for (index, negative) in negatives.iter().enumerate() {
        if control {
            if !negative.is_null() {
                return Err("callback control ran negatives");
            }
            continue;
        }
        if negative["control"] != serde_json::to_value(oracle::NEGATIVES[index]).unwrap()
            || negative["observed"] != observed::expected(oracle::NEGATIVES[index])
            || negative["helper_lane_mask"] != 0
            || negative["caller_lane_mask"] != 0
            || negative["global_writes"] != 0
            || negative["floor_restored"] != true
        {
            return Err("actual request refusal or delivered observation differs");
        }
    }
    Ok(())
}
#[test]
fn helper_cpu_acceptance_does_not_count_transport_or_normal_flags_as_execution() {
    for report in [
        json!({"stage":"actual_borrowed_transport_then_ordinary_refusal","diagnostic":"old transport"}),
        json!({"stage":"actual_helper_source_cpu_observed","diagnostic":"BF16 consumer unavailable","cpu":{}}),
        Value::Null,
    ] {
        assert!(accept("identity", &report).is_err());
        assert!(accept("swap01", &report).is_err());
    }
    assert!(accept("identity,swap01", &Value::Null).is_err());
}
