//! Opt-in actual-source normal ladder. No fixture bytes become source custody.
use super::{
    CRATE_NAME, Callbacks, Compilation, Compiler, FEATURES, PACKAGE, TyCtxt, checked,
    checked_feature, digest, expected_rejection, fixture, inputs, publish_json, read_bounded,
    repository, sanitized, timely,
};
use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "gfx942_ordered_composition_normal_inputs_v1_tests.rs"]
mod invocation;
#[path = "gfx942_ordered_composition_normal_observation_v1_tests.rs"]
mod observation;
const OUTPUT: &str = "FE2O3_TEST_ORDERED_COMPOSITION_NORMAL_OUTPUT_V1";
const INPUT: &str = "FE2O3_TEST_ORDERED_COMPOSITION_NORMAL_INPUT_V1";
const FEATURE: &str = "FE2O3_TEST_ORDERED_COMPOSITION_NORMAL_FEATURE_V1";
const FINITE: &str = "FE2O3_TEST_ORDERED_COMPOSITION_NORMAL_FINITE_V1";
const MODE: &str = "FE2O3_TEST_ORDERED_COMPOSITION_NORMAL_MODE_V1";
const CHILD: &str = "production_rustc_driver_v1::gfx942_ordered_composition_qualification_v1_tests::normal::actual_ordered_composition_normal_child";
const PREFIX: &str = "FE2O3_ORDERED_COMPOSITION_NORMAL_OBSERVATION_V1 ";
const DYNAMIC_REFUSAL: &str = "normal ordered composition requires explicit finite source max_grid";
fn case(feature: &str, finite: bool, mode: &str) -> Result<String, &'static str> {
    checked_feature(feature)?;
    if !matches!(mode, "observe" | "llvm" | "handoff")
        || (!finite && !FEATURES[..7].contains(&feature))
        || ((!finite || !FEATURES[..7].contains(&feature)) && mode != "observe")
    {
        return Err("unsupported actual normal case");
    }
    Ok(format!(
        "{feature}.{}.{}",
        if finite { "finite" } else { "dynamic" },
        mode
    ))
}
struct Body<'a> {
    feature: &'a str,
    output: &'a Path,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for Body<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            let transaction = super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let target = transaction.lower_ordered_composition_target_v1()?;
            observation::observe(target, self.feature, self.output)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "isolated actual rustc child; run actual_ordered_composition_normal_ladder"]
fn actual_ordered_composition_normal_child() {
    let started = std::time::Instant::now();
    let directory = PathBuf::from(std::env::var_os(INPUT).unwrap());
    assert!(directory.is_absolute());
    let feature = std::env::var(FEATURE).unwrap();
    let finite = match std::env::var(FINITE).unwrap().as_str() {
        "0" => false,
        "1" => true,
        _ => panic!("finite flag"),
    };
    let mode = std::env::var(MODE).unwrap();
    let name = case(&feature, finite, &mode).unwrap();
    let actual = invocation::derive(&directory, &feature, finite);
    let retained: invocation::Invocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{name}.invocation.json")),
            256 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, retained);
    for (key, value) in [
        (CRATE_BINDING_ID_ENV_V1, actual.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            actual.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", PACKAGE),
        ("CARGO_PKG_VERSION", "0.1.0"),
        ("CARGO_CRATE_NAME", CRATE_NAME),
        ("FE2O3_EXTRACT_ORDERED_COMPOSITION_V1", "1"),
    ] {
        assert_eq!(std::env::var(key).unwrap(), value);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        fixture()
    );
    let output = directory.join(format!("{name}.output"));
    timely(started.elapsed(), 300).unwrap();
    let result=match mode.as_str(){
        "observe"=>{
            let mut body=Body{feature:&feature,output:&output,calls:0,result:None};
            rustc_driver::run_compiler(&actual.args,&mut body);
            assert_eq!(body.calls,1);
            body.result.expect("actual source callback")
        },
        "llvm"=>super::super::run_production_gfx942_llvm_extraction_driver_v1(&actual.args,&output)
            .map(|()|json!({"stage":"public_normal_llvm","llvm_sha256":digest(&read_bounded(&output,4*1024*1024).unwrap())})),
        "handoff"=>super::super::run_production_gfx942_compiler_handoff_extraction_driver_v1(&actual.args,&output)
            .map(|()|{
                let bytes=read_bounded(&output,4*1024*1024).unwrap();
                let h=fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(&bytes).unwrap();
                assert!(!h.authenticates_compiler_origin()&&!h.grants_compiler_authority());
                json!({"stage":"public_normal_handoff","handoff_sha256":digest(&bytes)})
            }),
        _=>unreachable!(),
    };
    if let Err(error) = &result {
        assert!(error.len() <= 64 * 1024);
        publish_json(
            &directory,
            &format!("{name}.rejection.json"),
            &json!({"diagnostic":error,"accepted":false}),
        );
    }
    let observed = if finite && FEATURES[..7].contains(&feature.as_str()) {
        result.unwrap()
    } else {
        let diagnostic = result.expect_err("invalid source admitted");
        if finite {
            expected_rejection(&feature, &diagnostic).unwrap();
        } else {
            assert!(
                diagnostic.contains(DYNAMIC_REFUSAL),
                "wrong refusal: {diagnostic}"
            );
        }
        assert!(!output.exists());
        json!({"stage":if finite{"source_refused"}else{"dynamic_source_refused"},"diagnostic":diagnostic})
    };
    assert_eq!(invocation::derive(&directory, &feature, finite), actual);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    timely(started.elapsed(), 300).unwrap();
    let frame = serde_json::to_string(&json!({
        "schema":"fe2o3-test-composition-normal-session-v1","case":name,"feature":feature,
        "finite":finite,"mode":mode,"invocation":actual,"observation":observed,
        "runtime_conditions_discharged":false,"source_custody_exported":false,
        "native_llvm_executed":false,"hardware_observed":false,"protected_authority":false,
    }))
    .unwrap();
    assert!(frame.len() <= 256 * 1024);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
    timely(started.elapsed(), 300).unwrap();
}
fn prepare(directory: &Path, started: std::time::Instant) {
    let rustc = PathBuf::from(std::env::var_os("RUSTC").expect("absolute pinned RUSTC"));
    assert!(rustc.is_absolute());
    let mut command = Command::new(rustc);
    let bytes = checked(
        sanitized(&mut command).args(["--print", "sysroot"]),
        directory,
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
    let mut command = Command::new(sysroot.join("bin/cargo"));
    checked(
        sanitized(&mut command)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(fixture().join("Cargo.toml")),
        directory,
        "metadata",
        None,
    );
    timely(started.elapsed(), 2400).unwrap();
    let target = directory.join("dependencies");
    let mut command = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut command).current_dir(repository()).args([
        "check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device",
        "--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path",
    ]).arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&target)
        .env("RUSTC",sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        directory,"dependencies",Some(&target));
    fs::create_dir(directory.join("analysis-output")).unwrap();
    timely(started.elapsed(), 2400).unwrap();
}
fn session(
    directory: &Path,
    feature: &str,
    finite: bool,
    mode: &str,
    started: std::time::Instant,
) -> Value {
    let session_started = std::time::Instant::now();
    let name = case(feature, finite, mode).unwrap();
    let actual = invocation::derive(directory, feature, finite);
    publish_json(directory, &format!("{name}.invocation.json"), &actual);
    let mut child = Command::new(std::env::current_exe().unwrap());
    timely(started.elapsed(), 2400).unwrap();
    let bytes = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .args(["--exact", CHILD, "--ignored", "--nocapture"])
            .env(INPUT, directory)
            .env(FEATURE, feature)
            .env(FINITE, if finite { "1" } else { "0" })
            .env(MODE, mode)
            .env("FE2O3_EXTRACT_ORDERED_COMPOSITION_V1", "1")
            .env(CRATE_BINDING_ID_ENV_V1, &actual.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &actual.cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        &name,
        None,
    );
    timely(session_started.elapsed(), 300).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let rows = text
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let result: Value = serde_json::from_str(rows[0]).unwrap();
    assert_eq!(result["schema"], "fe2o3-test-composition-normal-session-v1");
    assert_eq!(result["case"], name);
    assert_eq!(result["feature"], feature);
    assert_eq!(result["finite"], finite);
    assert_eq!(result["mode"], mode);
    assert_eq!(result["invocation"], serde_json::to_value(&actual).unwrap());
    assert_eq!(invocation::derive(directory, feature, finite), actual);
    for flag in [
        "runtime_conditions_discharged",
        "source_custody_exported",
        "native_llvm_executed",
        "hardware_observed",
        "protected_authority",
    ] {
        assert_eq!(result[flag], false);
    }
    timely(session_started.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    publish_json(directory, &format!("{name}.accepted.json"), &result);
    timely(session_started.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    result
}
#[test]
#[ignore = "serialized actual pinned rustc36-session normal ladder; fresh absolute output required"]
fn actual_ordered_composition_normal_ladder() {
    let started = std::time::Instant::now();
    let directory = PathBuf::from(std::env::var_os(OUTPUT).expect("fresh private output"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let sources = inputs::current_sources();
    prepare(&directory, started);
    let dependencies = inputs::dependency_snapshot(&directory).0;
    let mut observations = Vec::new();
    for feature in &FEATURES[..7] {
        let owner = session(&directory, feature, true, "observe", started);
        let llvm = session(&directory, feature, true, "llvm", started);
        let handoff = session(&directory, feature, true, "handoff", started);
        assert_eq!(
            owner["observation"]["llvm_sha256"],
            llvm["observation"]["llvm_sha256"]
        );
        assert_eq!(
            owner["observation"]["handoff_sha256"],
            handoff["observation"]["handoff_sha256"]
        );
        let output = directory.join(format!(
            "{}.output",
            case(feature, true, "observe").unwrap()
        ));
        for (name, field) in [
            ("canonical.ll", "llvm_sha256"),
            ("handoff-v2.bin", "handoff_sha256"),
            ("canonical-v17.bin", "canonical_sha256"),
            ("descriptor-v1.bin", "descriptor_sha256"),
        ] {
            assert_eq!(
                owner["observation"][field],
                digest(&read_bounded(&output.join(name), 4 * 1024 * 1024).unwrap())
            );
        }
        observations.extend([owner, llvm, handoff]);
        observations.push(session(&directory, feature, false, "observe", started));
    }
    for feature in &FEATURES[7..] {
        observations.push(session(&directory, feature, true, "observe", started));
    }
    assert_eq!(observations.len(), 36);
    assert_eq!(inputs::current_sources(), sources);
    assert_eq!(inputs::dependency_snapshot(&directory).0, dependencies);
    timely(started.elapsed(), 2400).unwrap();
    publish_json(
        &directory,
        "normal-observation.json",
        &json!({
            "schema":"fe2o3-test-composition-normal-ladder-v1","actual_source_sessions":36,
            "finite_checked_owner_sessions":7,"public_normal_llvm_sessions":7,"public_normal_handoff_sessions":7,
            "original_dynamic_source_refusals":7,"invalid_source_refusals":8,
            "same_source_normal_llvm_handoff_joins":7,"descriptor_extension_mutations":21,
            "sources":sources,"dependencies":dependencies,"observations":observations,
            "native_llvm_executed":false,"hardware_observed":false,"source_custody_exported":false,
            "runtime_conditions_discharged":false,"protected_authority":false,
            "cleanup_scope":"bounded inherited direct-child/process-group runner, not whole-family proof",
            "acceptance":"requires completed successful parent test, not this JSON",
        }),
    );
    timely(started.elapsed(), 2400).unwrap();
}
#[test]
fn normal_cases_do_not_silently_switch_source_profiles() {
    assert!(case(FEATURES[0], true, "llvm").is_ok());
    assert!(case(FEATURES[0], false, "observe").is_ok());
    for mode in ["llvm", "handoff"] {
        assert!(case(FEATURES[0], false, mode).is_err());
        assert!(case(FEATURES[7], true, mode).is_err());
    }
    assert!(case("ordered-composition-finite-grid", true, "observe").is_err());
    assert!(case(FEATURES[0], true, "native").is_err());
}
#[test]
fn normal_ladder_final_deadline_does_not_accept_late_observation() {
    use std::time::Duration;
    for seconds in [300, 2400] {
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
