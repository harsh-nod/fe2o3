//! Separate actual-source candidate edit/re-admission callbacks; no report input.
use super::*;
use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;

#[path = "source_candidate_debug_join_driver_v17_tests.rs"]
mod debug_join;
#[path = "source_bitselect_candidate_machine_fixture_v1_tests.rs"]
mod fixture_cases;
#[path = "source_bitselect_headless_machine_v1_tests.rs"]
mod headless_machine;
#[path = "source_bitselect_candidate_machine_ladder_v1_tests.rs"]
mod ladder;
#[path = "source_candidate_launch_analysis_driver_v17_tests.rs"]
mod launch_analysis;

const OUTPUT: &str = "FE2O3_TEST_SOURCE_CANDIDATE_MACHINE_OUTPUT";
const INPUT: &str = "FE2O3_TEST_SOURCE_CANDIDATE_MACHINE_INPUT";
const SELECTOR: &str = "FE2O3_TEST_SOURCE_CANDIDATE_MACHINE_CASE";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::actual_source_candidate_machine_child";
const PREFIX: &str = "FE2O3_SOURCE_CANDIDATE_MACHINE ";
const SELECTORS: [&str; 6] = [
    "default",
    "edited",
    "repeat",
    "wrong-plan",
    "wrong-output",
    "stale-edited",
];

fn source_files(selector: &str) -> (&'static str, &'static str) {
    match selector {
        "default" | "wrong-plan" => ("candidate.rs", "candidate-loader.rs"),
        "edited" | "repeat" => ("edited.rs", "edited-loader.rs"),
        "wrong-output" => ("wrong-output.rs", "wrong-output-loader.rs"),
        "stale-edited" => ("stale-edited.rs", "stale-edited-loader.rs"),
        _ => panic!("closed candidate-machine selector"),
    }
}

fn register_plan(selector: &str) -> Gfx942OrderedProgramRegistersV1 {
    if selector == "default" {
        Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).unwrap()
    } else {
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap()
    }
}

fn expected_refusal(selector: &str) -> Option<&'static str> {
    match selector {
        "wrong-plan" => Some("source-candidate fresh program/role binding differs"),
        "wrong-output" => Some(
            "source-candidate machine oracle output, initialization, canaries or identity mismatch",
        ),
        "stale-edited" => Some("source-candidate retained bytes differ from parsed compiler input"),
        _ => None,
    }
}

fn derive(directory: &Path, selector: &str) -> RoundtripInvocation {
    assert!(SELECTORS.contains(&selector));
    let case_dir = paths::absolute_case(directory, "positive");
    let (source, loader) = source_files(selector);
    paths::checked_source_file(&case_dir.join(source));
    paths::checked_source_file(&case_dir.join(loader));
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &case_dir.join(loader),
        "gfx942",
    );
    RoundtripInvocation {
        case: "positive".into(),
        phase: selector.into(),
        source_directory: paths::relative_case(directory, "positive"),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: hash(&case_dir.join(source)),
        loader_sha256: hash(&case_dir.join(loader)),
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture().join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}

struct MachineCallbacks {
    source: Option<RetainedInput>,
    plan: Gfx942OrderedProgramRegistersV1,
    calls: usize,
    result: Option<Result<(Value, String), String>>,
}

impl Callbacks for MachineCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let input = self.source.take().expect("one move-only candidate source");
        self.result = Some(
            super::super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| {
                transaction.observe_source_bitselect_candidate_machine(input, self.plan)
            }),
        );
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual rustc child; use actual_source_candidate_machine_ladder"]
fn actual_source_candidate_machine_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("prepared output"));
    let selector = std::env::var(SELECTOR).expect("closed case");
    require_current_source();
    let actual = derive(&directory, &selector);
    let retained: RoundtripInvocation = serde_json::from_slice(
        &read_bounded(
            &directory
                .join("positive")
                .join(format!("{selector}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, retained,
        "current source/loader/dependency invocation"
    );
    assert_eq!(std::env::current_dir().unwrap(), repository());
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
    let source_path = actual.source_directory.join(source_files(&selector).0);
    let source = RetainedInput::open(source_path.to_str().unwrap(), true).unwrap();
    if selector == "stale-edited" {
        // Intentionally mutate only this fresh task-owned negative after retention.
        fs::OpenOptions::new()
            .append(true)
            .open(&source_path)
            .unwrap()
            .write_all(b"\n// actual edited file changed before rustc parse\n")
            .unwrap();
    }
    let mut callbacks = MachineCallbacks {
        source: Some(source),
        plan: register_plan(&selector),
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(
        callbacks.calls, 1,
        "negative must reach its intended actual callback"
    );
    let result = callbacks.result.expect("actual source callback result");
    let llvm_path = directory.join("positive").join(format!("{selector}.ll"));
    assert!(!llvm_path.exists(), "fresh diagnostic output");
    let observation = if let Some(expected) = expected_refusal(&selector) {
        let diagnostic = result.expect_err("specific actual candidate negative");
        assert_eq!(
            diagnostic, expected,
            "no arbitrary compiler failure accepted"
        );
        json!({"stage":"actual_candidate_machine_refused","diagnostic":diagnostic})
    } else {
        let (observation, llvm) = result.unwrap();
        assert_eq!(observation["fresh"]["kernel_ir_version"], "V17");
        assert_eq!(observation["fresh"]["semantic_version"], "V32");
        assert_eq!(observation["fresh"]["boolean_oracle_cases"], 128);
        assert_eq!(observation["whole_kernel_simulation"]["runs"], 30);
        assert_eq!(observation["old_evidence_reused"], false);
        assert_eq!(observation["native_emitted"], false);
        assert!(llvm.len() <= 64 * 1024);
        assert_eq!(observation["llvm_bytes"], llvm.len());
        assert_eq!(
            observation["llvm_sha256"],
            json!(<[u8; 32]>::from(Sha256::digest(llvm.as_bytes())))
        );
        // The returned bytes are inert diagnostics; no source owner is recreated.
        // This test output is create-new and read back before its success report.
        paths::write_new(&llvm_path, llvm.as_bytes());
        assert_eq!(
            read_bounded(&llvm_path, 64 * 1024).unwrap(),
            llvm.as_bytes()
        );
        assert_eq!(hash(&source_path), actual.source_sha256);
        observation
    };
    if expected_refusal(&selector).is_some() {
        assert!(!llvm_path.exists());
    }
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = serde_json::to_vec(&json!({
        "invocation":actual,"observation":observation,"actual_rustc_callback":true,
        "llvm_path":if expected_refusal(&selector).is_none() {Some(llvm_path)} else {None},
        "production_resume":false,"native_qualified":false,
        "hardware_observed":false,"grants_artifact_or_launch_authority":false,
    }))
    .unwrap();
    assert!(report.len() <= 64 * 1024);
    println!("\n{PREFIX}{}", std::str::from_utf8(&report).unwrap());
}

fn run_machine_child(directory: &Path, selector: &str) -> Value {
    let record = derive(directory, selector);
    paths::write_new(
        &directory
            .join("positive")
            .join(format!("{selector}.invocation.json")),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut child = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .args(["--exact", CHILD, "--ignored", "--nocapture"])
            .env(INPUT, directory)
            .env(SELECTOR, selector)
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
        &format!("machine-{selector}"),
        None,
    );
    let stdout = std::str::from_utf8(&stdout).unwrap();
    let mut lines = stdout.lines().filter_map(|line| line.strip_prefix(PREFIX));
    let line = lines.next().expect("single observation");
    assert!(line.len() <= 64 * 1024);
    assert!(lines.next().is_none());
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let report: Value = serde_json::from_str(line).unwrap();
    assert_eq!(report["invocation"], serde_json::to_value(record).unwrap());
    report
}
