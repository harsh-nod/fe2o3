//! Fresh actual-source CPU gate. Historical inspection/normal reports remain unchanged.
use super::*;
#[path = "gfx942_tiled_region_cpu_capture_v1_tests.rs"]
pub(in crate::production_rustc_driver_v1) mod capture;
#[path = "gfx942_tiled_region_cpu_observation_v1_tests.rs"]
mod observed;
#[path = "gfx942_tiled_region_cpu_oracle_v1_tests.rs"]
pub(in crate::production_rustc_driver_v1) mod oracle;

const CPU_OUTPUT: &str = "FE2O3_TEST_TILED_CPU_OUTPUT_V1";
const CPU_SCHEMA: &str = "fe2o3-gfx942-bf16-source-cpu-observation-v1";
const CPU_PREFIX: &str = "FE2O3_GFX942_BF16_SOURCE_CPU_OBSERVATION_V1 ";
const CPU_CHILD: &str = "production_rustc_driver_v1::gfx942_tiled_region_qualification_v1_tests::observation::cpu::actual_bf16_source_cpu_child";
const CPU_CASES: [&str; 4] = ["direct", "wrong-launch", "direct-error", "direct-panic"];
const FRAME_CAP: usize = 256 * 1024;

struct CpuCallbacks<'a> {
    case: &'a str,
    calls: usize,
    result: Option<Value>,
}
impl Callbacks for CpuCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(
            match crate::production_rustc_driver_v1::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            ) {
                Ok(transaction) => observed::observe(transaction, self.case),
                Err(error) => json!({"stage":"actual_source_cpu_refused","diagnostic":error,
                "phase":Value::Null,"collection_refused":true}),
            },
        );
        Compilation::Stop
    }
}
fn accept_cpu(case: &str, row: &Value) -> Result<(), &'static str> {
    if !CPU_CASES.contains(&case) {
        return Err("unknown CPU source case");
    }
    if case == "wrong-launch" {
        if row["stage"] != "actual_source_cpu_refused"
            || row["diagnostic"]
                .as_str()
                .is_none_or(|s| !s.contains("BF16 source requires explicit WG64 and one workgroup"))
        {
            return Err("actual finite-launch source rejection differs");
        }
        return Ok(());
    }
    let phase = &row["phase"];
    if phase["same_ledger"] != true
        || phase["failed_work"] != false
        || phase["failed_storage"] != false
    {
        return Err("original source phase ledger was reset or denied");
    }
    let accounting = row["accounting"]
        .as_array()
        .ok_or("CPU phase accounting absent")?;
    if accounting.len() != 5 {
        return Err("CPU accounting shape");
    }
    let n = |i: usize| accounting[i].as_u64().ok_or("CPU accounting number");
    if n(1)? != n(0)?.checked_add(n(4)?).ok_or("CPU accounting overflow")? || n(3)? <= n(2)? {
        return Err("same source floor or cumulative work differs");
    }
    if case != "direct" {
        let expected_final = phase["materializer_storage"]
            .as_u64()
            .and_then(|v| v.checked_sub(phase["source_storage"].as_u64()?))
            .and_then(|v| v.checked_add(n(4).ok()?));
        if expected_final.is_none() || phase["final_storage"].as_u64() != expected_final {
            return Err("actual refused materializer/source/retained CPU row relation");
        }
        let fragment = if case == "direct-error" {
            "source CPU observer error control"
        } else {
            "CallbackPanicked"
        };
        if row["stage"] != "actual_source_cpu_refused"
            || phase["result_ok"] != false
            || row["diagnostic"]
                .as_str()
                .is_none_or(|s| !s.contains(fragment))
        {
            return Err("wrong actual observer error/unwind boundary");
        }
        return Ok(());
    }
    let cpu = &row["cpu"];
    // Success additionally reserves the genuine returned pre-ranked owner and
    // occurrence receipt in the existing materializer. Those are NOT observer
    // scratch and must not be subtracted as if this were its error branch.
    if phase["final_storage"]
        .as_u64()
        .is_none_or(|v| v < n(4).unwrap_or(u64::MAX))
        || phase["work"]
            .as_u64()
            .is_none_or(|v| v < n(3).unwrap_or(u64::MAX))
    {
        return Err("successful original phase lost retained charge or work");
    }
    if row["stage"] != "actual_source_graph_cpu"
        || phase["result_ok"] != true
        || cpu["attempted_runs"] != 34
        || cpu["same_original_ledger"] != true
        || cpu["source_authority_in_copied_row"] != false
    {
        return Err("fresh actual whole graph did not qualify");
    }
    let runs = cpu["runs"].as_array().ok_or("actual run rows absent")?;
    let negatives = cpu["negatives"]
        .as_array()
        .ok_or("actual refusal rows absent")?;
    if runs.len() != 18 || negatives.len() != 16 || runs.iter().chain(negatives).any(Value::is_null)
    {
        return Err("incomplete actual numerical matrix");
    }
    for (index, run) in runs.iter().enumerate() {
        let length = oracle::LENGTHS[index % 3];
        if run["pattern"] != index / 3
            || run["output_length"] != length
            || run["matrix_lane_mask"].as_u64() != Some(u64::MAX)
            || run["storage_floor"] != run["storage_after"]
            || run["work_after"]
                .as_u64()
                .zip(run["work_before"].as_u64())
                .is_none_or(|(a, b)| a <= b)
        {
            return Err("actual positive row identity or accounting differs");
        }
        if run["values_row_major_le_hex"]
            != serde_json::to_value(oracle::expected(index / 3)).unwrap()
        {
            return Err("retained actual SSA result differs from independent oracle");
        }
        if run["output_with_canaries_le_hex"]
            != serde_json::to_value(oracle::expected_output(index / 3, length)).unwrap()
        {
            return Err("retained actual memory sink or canaries differ");
        }
    }
    for (actual, control) in negatives.iter().zip(oracle::NEGATIVES) {
        if actual["control"] != serde_json::to_value(control).unwrap()
            || actual["observed"] != capture::expected_refusal(control)
            || actual["matrix_lane_mask"] != 0
            || actual["global_writes"] != 0
            || actual["floor_restored"] != true
        {
            return Err("actual precise refusal control differs");
        }
    }
    Ok(())
}
fn publish_cpu(directory: &Path, name: &str, value: &impl Serialize) {
    let bytes = serde_json::to_vec_pretty(value).unwrap();
    assert!(bytes.len() <= FRAME_CAP);
    crate::production_rustc_driver_v1::publish_new_inert_output(
        &directory.join(name),
        &bytes,
        FRAME_CAP,
        "actual BF16 source CPU observation",
    )
    .unwrap();
}
#[test]
#[ignore = "pinned actual-rustc child; use actual_bf16_source_cpu_ladder"]
fn actual_bf16_source_cpu_child() {
    let started = std::time::Instant::now();
    // Separate test-output lifetime, prepaid BEFORE the compiler callback. This
    // meter is never supplied to a source owner, admission, or simulator API.
    // The source phase still receives its original existing Budget unchanged.
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = Budget::new(&mut copied_work, 64 * 1024 + FRAME_CAP);
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("actual preparation directory"));
    assert!(directory.is_absolute());
    let case = std::env::var(CASE_ENV).unwrap();
    assert!(CPU_CASES.contains(&case.as_str()));
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
    assert_eq!(actual, retained);
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
    crate::production_rustc_driver_v1::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let mut callbacks = CpuCallbacks {
        case: &case,
        calls: 0,
        result: None,
    };
    timely(started.elapsed(), 300).unwrap();
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let observed = callbacks.result.unwrap();
    // Preserve actual first failure/census BEFORE any success oracle.
    publish_cpu(
        &directory,
        &format!("{case}.cpu-observed.json"),
        &json!({"case":case,"observation":observed,"accepted":false}),
    );
    accept_cpu(&case, &observed).unwrap();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(inputs::derive_record(&directory, feature), actual);
    timely(started.elapsed(), 300).unwrap();
    let frame = json!({"schema":CPU_SCHEMA,"case":case,"feature":feature,"invocation":actual,
        "observation":observed,"actual_rustc_callbacks":1,"source_and_dependencies_unchanged":true,
        "numerical_cpu_qualified":case=="direct","pre_ranked_only":true,
        "normal_ranked_formal_target_handoff_qualified":false,"source_authority_in_report":false,
        "native_execution_attempted":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false});
    let encoded = serde_json::to_string(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{CPU_PREFIX}{encoded}");
    timely(started.elapsed(), 300).unwrap();
    // Encoded text and fixed inert copies are dropped before this separate
    // test reservation ends; it is neither a reset nor transferred authority.
    drop(encoded);
    drop(frame);
    drop(observed);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
}
#[test]
#[ignore = "fresh actual source graph CPU qualification; creates new task output and runs four rustc children"]
fn actual_bf16_source_cpu_ladder() {
    let started = std::time::Instant::now();
    let directory = create_output(&PathBuf::from(
        std::env::var_os(CPU_OUTPUT).expect("fresh task output"),
    ));
    let sources = inputs::current_sources();
    let rustc_path =
        PathBuf::from(std::env::var_os("RUSTC").expect("absolute pinned-nightly RUSTC"));
    assert!(rustc_path.is_absolute());
    let bytes = checked(
        sanitized(&mut Command::new(rustc_path)).args(["--print", "sysroot"]),
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
    let bytes = checked(
        sanitized(&mut Command::new(sysroot.join("bin/cargo")))
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
    checked(sanitized(&mut Command::new(sysroot.join("bin/cargo"))).current_dir(repository()).args([
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
    let mut observations = Vec::with_capacity(4);
    for case in CPU_CASES {
        let child_started = std::time::Instant::now();
        let feature = feature_for_case(case).unwrap();
        let record = inputs::derive_record(&directory, feature);
        assert_eq!(record.sources, sources);
        assert_eq!(record.dependencies, dependencies);
        publish_json(&directory, &format!("{case}.invocation.json"), &record);
        timely(started.elapsed(), 1200).unwrap();
        let stdout = checked(
            sanitized(&mut Command::new(std::env::current_exe().unwrap()))
                .current_dir(repository())
                .args(["--exact", CPU_CHILD, "--ignored", "--nocapture"])
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
        let text = std::str::from_utf8(&stdout).unwrap();
        let mut frames = text
            .lines()
            .filter_map(|line| line.strip_prefix(CPU_PREFIX));
        let frame = frames.next().unwrap();
        assert!(frames.next().is_none() && frame.len() <= FRAME_CAP);
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let observed: Value = serde_json::from_str(frame).unwrap();
        assert_eq!(observed["schema"], CPU_SCHEMA);
        assert_eq!(observed["case"], case);
        assert_eq!(observed["feature"], feature);
        assert_eq!(
            observed["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observed["actual_rustc_callbacks"], 1);
        assert_eq!(observed["source_and_dependencies_unchanged"], true);
        assert_eq!(observed["numerical_cpu_qualified"], case == "direct");
        for flag in [
            "normal_ranked_formal_target_handoff_qualified",
            "source_authority_in_report",
            "native_execution_attempted",
            "hardware_observed",
            "grants_artifact_or_launch_authority",
        ] {
            assert_eq!(observed[flag], false);
        }
        accept_cpu(case, &observed["observation"]).unwrap();
        let raw: Value = serde_json::from_slice(
            &read_bounded(
                &directory.join(format!("{case}.cpu-observed.json")),
                FRAME_CAP,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["observation"], observed["observation"]);
        assert_eq!(raw["accepted"], false);
        assert_eq!(inputs::derive_record(&directory, feature), record);
        timely(child_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        publish_cpu(&directory, &format!("{case}.cpu-accepted.json"), &observed);
        timely(child_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        observations.push(observed);
    }
    assert_eq!(inputs::current_sources(), sources);
    assert_eq!(inputs::dependency_snapshot(&directory).0, dependencies);
    timely(started.elapsed(), 1200).unwrap();
    let report = json!({"schema":CPU_SCHEMA,"gate":"fresh-source-graph-cpu-four-sessions",
        "actual_rustc_sessions":4,"observations":observations,"source_files":sources,
        "dependency_snapshot":dependencies,"numerical_cpu_qualified":true,
        "positive_cases":18,"request_refusals":16,"source_launch_refusals":1,"observer_error_unwind_controls":2,
        "normal_ranked_formal_target_handoff_qualified":false,"native_execution_attempted":false,
        "grants_artifact_or_launch_authority":false,"source_authority_in_report":false,
        "cleanup_scope":"reused bounded direct-child/process-group helper, not whole-family supervision",
        "acceptance":"completed successful parent test required; inert copied report only"});
    publish_cpu(&directory, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
fn cpu_report_cannot_be_satisfied_by_historical_source_or_normal_flags() {
    for value in [
        json!({"stage":"actual_pre_ranked_source_region","numerical_cpu_qualified":false}),
        json!({"stage":"actual_normal_handoff","normal_ranked_formal_target_handoff_qualified":true}),
        json!({"stage":"actual_source_graph_cpu","phase":{"same_ledger":false}}),
    ] {
        assert!(accept_cpu("direct", &value).is_err());
    }
    assert!(accept_cpu("direct,normal", &Value::Null).is_err());
}
