//! Actual callbacks for two explicit local preferences through fixed-prefix L.
//! Reuses source preparation and exact negative fixtures, not their observations.
use super::*;
use fe2o3_kernel_opt::U32LocalOrderPreferenceV1 as Preference;

const OUTPUT: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_CONTINUATION_OUTPUT";
const ORDER: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_CONTINUATION_ORDER";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::local_order::continuation::actual_source_local_order_continuation_child";
const PREFIX: &str = "FE2O3_SOURCE_LOCAL_ORDER_CONTINUATION ";

fn preference(order: &str) -> Preference {
    match order {
        "source-order" => Preference::SourceOrder,
        "reverse-ready" => Preference::ReverseReady,
        _ => panic!("closed local-order continuation preference"),
    }
}
fn stem(case: &str, order: &str, run: &str) -> String {
    assert!(fixture_cases::CASES.contains(&case));
    preference(order);
    assert!(matches!(run, "first" | "repeat"));
    format!("continuation-{case}-{order}-{run}")
}
struct ContinuationCallbacks {
    source: Option<RetainedInput>,
    preference: Preference,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for ContinuationCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let source = self.source.take().expect("one retained source descriptor");
        self.result = Some(
            crate::production_rustc_driver_v1::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| {
                transaction.observe_source_local_order_continuation_v1(source, self.preference)
            }),
        );
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual rustc child; use actual_source_local_order_continuation_ladder"]
fn actual_source_local_order_continuation_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("continuation input"));
    let case = std::env::var(CASE).expect("continuation source case");
    let run = std::env::var(RUN).expect("continuation source run");
    let order = std::env::var(ORDER).expect("continuation preference");
    let stem = stem(&case, &order, &run);
    require_current_source();
    assert_eq!(std::env::current_dir().unwrap(), repository());
    let actual = derive(&directory, &case, &run);
    let saved: Invocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{stem}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, saved, "stale continuation invocation");
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
    crate::production_rustc_driver_v1::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let source = RetainedInput::open(actual.source_relative.to_str().unwrap(), false).unwrap();
    if case == "stale-source" {
        fs::OpenOptions::new()
            .append(true)
            .open(&actual.source_relative)
            .unwrap()
            .write_all(b"\n// changed after descriptor retention, before actual rustc\n")
            .unwrap();
    }
    let mut callbacks = ContinuationCallbacks {
        source: Some(source),
        preference: preference(&order),
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let result = callbacks
        .result
        .expect("actual continuation callback absent");
    let observation = if let Some(expected) = fixture_cases::refusal(&case) {
        let diagnostic = result.expect_err("source negative must reach its exact boundary");
        assert_eq!(diagnostic, expected);
        json!({"stage":"actual_source_local_order_continuation_refused","diagnostic":diagnostic})
    } else {
        let result = result.unwrap();
        assert_eq!(
            result["stage"],
            "actual_source_local_order_owned_continuation"
        );
        assert_eq!(result["preference"], order);
        assert_eq!(result["parameter_ordinals"], json!([1, 2, 3, 4]));
        assert_eq!(result["simulation"]["scenarios"], 15);
        assert_eq!(result["simulation"]["runs"], 30);
        assert_eq!(result["simulation"]["output_and_canaries_checked"], true);
        assert_eq!(result["actual_output_used_for_llvm"], true);
        assert_eq!(
            result["descriptor_producer"],
            "source-local-order-policy6-v1/gfx942"
        );
        assert_eq!(result["final_source_output_admitted"], true);
        assert_eq!(result["persistent_recipe_admitted"], false);
        assert_eq!(result["existing_policy6_modified"], false);
        assert_eq!(result["native_object_emitted"], false);
        assert_eq!(result["grants_artifact_or_launch_authority"], false);
        result
    };
    if case != "stale-source" {
        assert_eq!(hash(&actual.source_relative), actual.source_sha256);
    }
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = serde_json::to_vec(&json!({
        "invocation":actual,"observation":observation,
        "actual_rustc_callback":true,"source_produced":true,
        "grants_artifact_or_launch_authority":false
    }))
    .unwrap();
    assert!(report.len() <= 128 * 1024);
    println!("\n{PREFIX}{}", std::str::from_utf8(&report).unwrap());
}

fn child(directory: &Path, case: &str, order: &str, run: &str) -> Value {
    let stem = stem(case, order, run);
    let record = derive(directory, case, run);
    paths::write_new(
        &directory.join(format!("{stem}.invocation.json")),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        sanitized(&mut command)
            .current_dir(repository())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(INPUT, directory)
            .env(CASE, case)
            .env(RUN, run)
            .env(ORDER, order)
            .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &record.cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        &stem,
        None,
    );
    let stdout = std::str::from_utf8(&stdout).unwrap();
    let reports = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].len() <= 128 * 1024);
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let report: Value = serde_json::from_str(reports[0]).unwrap();
    assert_eq!(report["invocation"], serde_json::to_value(record).unwrap());
    if fixture_cases::refusal(case).is_none() {
        let llvm = report["observation"]["llvm_text"].as_str().unwrap();
        assert!(llvm.len() <= 48 * 1024);
        paths::write_new(&directory.join(format!("{stem}.ll")), llvm.as_bytes());
    }
    report
}

#[test]
#[ignore = "pinned actual source, serialized Cargo, fresh absolute output directory"]
fn actual_source_local_order_continuation_ladder() {
    let directory = PathBuf::from(std::env::var_os(OUTPUT).expect("fresh continuation output"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    super::super::prepare(&directory);
    let mut observations = Vec::new();
    let mut source_bytes = 0usize;
    for case in fixture_cases::CASES {
        let case_dir = source_root.join(case);
        fs::create_dir(&case_dir).unwrap();
        paths::write_new(
            &case_dir.join("source.rs"),
            fixture_cases::source(case).as_bytes(),
        );
        let first = child(&directory, case, "source-order", "first");
        if fixture_cases::refusal(case).is_none() {
            let reverse = child(&directory, case, "reverse-ready", "first");
            assert_eq!(
                first["observation"]["input_identity"],
                reverse["observation"]["input_identity"]
            );
            assert_eq!(
                first["observation"]["output_identity"],
                first["observation"]["input_identity"]
            );
            assert_ne!(
                first["observation"]["output_identity"],
                reverse["observation"]["output_identity"]
            );
            assert_ne!(
                first["observation"]["output_order"],
                reverse["observation"]["output_order"]
            );
            assert_ne!(
                first["observation"]["llvm_sha256"],
                reverse["observation"]["llvm_sha256"]
            );
            if case == "positive" {
                let repeated = child(&directory, case, "reverse-ready", "repeat");
                assert_eq!(reverse["observation"], repeated["observation"]);
                observations.push(repeated);
            }
            observations.push(reverse);
        }
        observations.push(first);
        source_bytes += fs::metadata(fixture_cases::absolute(&directory, case))
            .unwrap()
            .len() as usize;
        assert_eq!(fs::read_dir(case_dir).unwrap().take(2).count(), 1);
    }
    assert_eq!(observations.len(), 13);
    assert_eq!(
        observations
            .iter()
            .filter(
                |row| row["observation"]["stage"] == "actual_source_local_order_owned_continuation"
            )
            .count(),
        5
    );
    assert_eq!(
        observations
            .iter()
            .filter(|row| row["observation"]["stage"]
                == "actual_source_local_order_continuation_refused")
            .count(),
        8
    );
    assert_eq!(fs::read_dir(&source_root).unwrap().take(11).count(), 10);
    assert!(source_bytes <= 10 * 64 * 1024);
    require_current_source();
    let report = serde_json::to_vec_pretty(&json!({
        "observations":observations,"source_directory":paths::relative_root(&directory),
        "source_files":10,"source_bytes":source_bytes,"actual_callbacks":13,
        "positive_callbacks":5,"refused_callbacks":8,"simulation_runs":150,
        "source_and_reverse_orders":true,"fresh_repeated_capture":true,
        "persistent_recipe_admitted":false,"existing_policy6_modified":false,
        "final_source_output_admitted":true,"native_object_emitted":false,
        "hardware_observed":false,"grants_artifact_or_launch_authority":false
    }))
    .unwrap();
    assert!(report.len() <= 1024 * 1024);
    paths::write_new(&directory.join("observation.json"), &report);
}
