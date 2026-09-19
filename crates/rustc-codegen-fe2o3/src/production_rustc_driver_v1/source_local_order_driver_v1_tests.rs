//! Actual rustc callbacks only; no production schedule selector or recipe schema.
//! Intended child of source_bitselect_feasibility_v1_tests::roundtrip.
use super::*;

#[path = "source_local_order_fixture_v1_tests.rs"]
mod fixture_cases;

const OUTPUT: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_OUTPUT";
const INPUT: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_INPUT";
const CASE: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_CASE";
const RUN: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RUN";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::local_order::actual_source_local_order_child";
const PREFIX: &str = "FE2O3_SOURCE_LOCAL_ORDER ";

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    case: String,
    run: String,
    source_relative: PathBuf,
    source_sha256: String,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
    fixture_sha256: [String; 4],
    artifacts_sha256: String,
    metadata_sha256: String,
}

fn derive(directory: &Path, case: &str, run: &str) -> Invocation {
    assert!(fixture_cases::CASES.contains(&case));
    assert!(matches!(run, "first" | "repeat"));
    let source = fixture_cases::absolute(directory, case);
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &source,
        if case == "wrong-target" {
            "gfx950"
        } else {
            "gfx942"
        },
    );
    Invocation {
        case: case.into(),
        run: run.into(),
        source_relative: fixture_cases::relative(directory, case),
        source_sha256: hash(&source),
        args,
        crate_binding,
        cargo_observation,
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture().join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}

struct LocalOrderCallbacks {
    source: Option<RetainedInput>,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for LocalOrderCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let source = self.source.take().expect("one retained source descriptor");
        self.result = Some(
            super::super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| transaction.observe_source_local_order_feasibility(source)),
        );
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual rustc child; use actual_source_local_order_feasibility_ladder"]
fn actual_source_local_order_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("local-order input directory"));
    let case = std::env::var(CASE).expect("local-order case");
    let run = std::env::var(RUN).expect("local-order run");
    require_current_source();
    assert_eq!(std::env::current_dir().unwrap(), repository());
    let actual = derive(&directory, &case, &run);
    let saved: Invocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{case}-{run}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, saved, "stale local-order invocation preparation");
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
    super::super::super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let source = RetainedInput::open(actual.source_relative.to_str().unwrap(), false).unwrap();
    if case == "stale-source" {
        // Only our fresh task-owned source changes. The retained pre-rustc bytes
        // must differ from the actual parsed SourceFile, at the exact guard.
        fs::OpenOptions::new()
            .append(true)
            .open(&actual.source_relative)
            .unwrap()
            .write_all(b"\n// changed after descriptor retention, before actual rustc\n")
            .unwrap();
    }
    let mut callbacks = LocalOrderCallbacks {
        source: Some(source),
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "actual callback count");
    let result = callbacks
        .result
        .expect("actual local-order callback did not run");
    let observation = if let Some(expected) = fixture_cases::refusal(&case) {
        let diagnostic = result.expect_err("source negative must refuse its exact local boundary");
        assert_eq!(
            diagnostic, expected,
            "an earlier/later compiler failure is not this negative"
        );
        json!({"stage":"actual_source_local_order_refused","diagnostic":diagnostic})
    } else {
        let result = result.unwrap();
        assert_eq!(
            result["stage"],
            "actual_source_bound_v12_local_order_diagnostic"
        );
        assert_eq!(result["connected_version"], "V12");
        assert_eq!(result["bound_version"], "V12");
        assert_eq!(result["scheduled_version"], "V12");
        assert_ne!(
            result["source_order_identity"],
            result["reverse_ready_identity"]
        );
        assert_ne!(result["source_order"], result["reverse_ready"]);
        assert_eq!(result["parameter_ordinals"], json!([1, 2, 3, 4]));
        assert_eq!(result["independent_transition_replays"], 3);
        assert_eq!(result["malformed_region_refusals"], 1);
        for kind in ["source_order_simulation", "reverse_ready_simulation"] {
            assert_eq!(result[kind]["scenarios"], 15);
            assert_eq!(result[kind]["runs"], 30);
            assert_eq!(result[kind]["output_and_canaries_checked"], true);
        }
        assert_eq!(result["fixed_production_policy_modified"], false);
        assert_eq!(result["persistent_recipe_admitted"], false);
        assert_eq!(result["final_source_output_admitted"], false);
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
    let report = serde_json::to_vec(&json!({"invocation":actual,"observation":observation,
        "actual_rustc_callback":true,"source_produced":true,"grants_artifact_or_launch_authority":false})).unwrap();
    assert!(report.len() <= 64 * 1024);
    println!("\n{PREFIX}{}", std::str::from_utf8(&report).unwrap());
}

fn child(directory: &Path, case: &str, run: &str) -> Value {
    let record = derive(directory, case, run);
    paths::write_new(
        &directory.join(format!("{case}-{run}.invocation.json")),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        sanitized(&mut command)
            .current_dir(repository())
            .args(["--exact", CHILD, "--ignored", "--nocapture"])
            .env(INPUT, directory)
            .env(CASE, case)
            .env(RUN, run)
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
        &format!("{case}-{run}"),
        None,
    );
    let stdout = std::str::from_utf8(&stdout).unwrap();
    let reports = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].len() <= 64 * 1024);
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let report: Value = serde_json::from_str(reports[0]).unwrap();
    assert_eq!(report["invocation"], serde_json::to_value(record).unwrap());
    report
}

#[test]
#[ignore = "pinned actual source, serialized Cargo, fresh absolute output directory"]
fn actual_source_local_order_feasibility_ladder() {
    let directory =
        PathBuf::from(std::env::var_os(OUTPUT).expect("fresh local-order output directory"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    super::prepare(&directory);
    let mut observations = Vec::new();
    let mut source_bytes = 0usize;
    for case in fixture_cases::CASES {
        let case_dir = source_root.join(case);
        fs::create_dir(&case_dir).unwrap();
        paths::write_new(
            &case_dir.join("source.rs"),
            fixture_cases::source(case).as_bytes(),
        );
        observations.push(child(&directory, case, "first"));
        if case == "positive" {
            let repeated = child(&directory, case, "repeat");
            assert_eq!(
                observations.last().unwrap()["observation"],
                repeated["observation"],
                "fresh frontend replay must be deterministic"
            );
            observations.push(repeated);
        }
        let source = fixture_cases::absolute(&directory, case);
        source_bytes = source_bytes
            .checked_add(fs::metadata(source).unwrap().len() as usize)
            .unwrap();
        assert_eq!(fs::read_dir(case_dir).unwrap().take(2).count(), 1);
    }
    assert_eq!(
        fs::read_dir(&source_root)
            .unwrap()
            .take(fixture_cases::CASES.len() + 1)
            .count(),
        fixture_cases::CASES.len()
    );
    assert!(source_bytes <= fixture_cases::CASES.len() * 64 * 1024);
    assert_eq!(observations.len(), 11);
    assert_ne!(
        observations[0]["observation"]["source_sha256"],
        observations[2]["observation"]["source_sha256"]
    );
    assert_ne!(
        observations[0]["observation"]["source_initializer"],
        observations[2]["observation"]["source_initializer"]
    );
    require_current_source();
    let report = serde_json::to_vec_pretty(&json!({"observations":observations,
        "source_directory":paths::relative_root(&directory),"source_files":10,"source_bytes":source_bytes,
        "source_file_limit":10,"source_byte_limit":10 * 64 * 1024,"actual_callbacks":11,
        "persistent_recipe_admitted":false,"fixed_production_policy_modified":false,
        "final_source_output_admitted":false,"native_emitted":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false})).unwrap();
    assert!(report.len() <= 512 * 1024);
    paths::write_new(&directory.join("observation.json"), &report);
}
