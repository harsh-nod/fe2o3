//! Literal fixed8 public-runner controls; old policy protocols remain unchanged.
use super::*;
use crate::production_pipeline::checked_output_policy8_v1::source_observation as live;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
const CHILD8: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::fixed_census_lifecycle::policy8::fixed8_census_lifecycle_child";
const REQUEST8: &str = "FE2O3_TEST_FIXED8_CENSUS_REQUEST_V1";
#[path = "production_rustc_driver_fixed8_boundary_v1_tests.rs"]
mod boundary;
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request8 {
    input: Input,
    target: String,
    diagnostic: Diagnostic,
    args_digest: [u8; 32],
    source: Vec<Stamp>,
    active_hash: [u8; 32],
    artifact: PathBuf,
    run_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Outcome8 {
    Success {},
    AdmissionFailure { error: String },
    Fatal {},
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Observed8 {
    calls: usize,
    roots: Option<Result<Vec<census::SourceRoot>, String>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Stage8 {
    calls: usize,
    erased: bool,
    original: [u8; 32],
    output: [u8; 32],
    llvm: [u8; 32],
}

fn stage8(
    view: live::View<'_>,
    input: Input,
    target: &str,
    budget: &mut Budget<'_>,
) -> Result<Stage8, String> {
    let erased = matches!(view.owner, live::Owner::Erased(_));
    if erased != (input == Input::RetainedUnit)
        || !input.succeeds()
        || view.owner.pairs() != 0
        || view.owner.historical_j().canonical().canonical_bytes()
            != view.owner.output().canonical().canonical_bytes()
        || view.artifacts.policy_version() != 8
        || view.artifacts.grants_artifact_or_launch_authority()
        || view.profile.device_target() != format!("{target}:xnack-")
        || view
            .artifacts
            .descriptor_source()
            .table()
            .producer()
            .version()
            .as_str()
            != format!("production-policy8-checked-{target}-cov6-v1")
        || view.ranked.roots().len() != 1
    {
        return Err("literal fixed8 route/no-op/target/producer/source roster".into());
    }
    if let live::Owner::Erased(owner) = view.owner
        && owner
            .prefix()
            .prefix()
            .erased()
            .canonical()
            .canonical_bytes()
            == view.owner.original().canonical().canonical_bytes()
    {
        return Err("retained UnitLocal source lost distinct actual N and E".into());
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    view.replay(budget)?;
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err("literal fixed8 replay changed live ledger/floor".into());
    }
    Ok(Stage8 {
        calls: 1,
        erased,
        original: *view.owner.original().canonical().identity().digest(),
        output: *view.owner.output().canonical().identity().digest(),
        llvm: digest(view.artifacts.llvm_ir().as_bytes()),
    })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report8 {
    request: Request8,
    outcome: Outcome8,
    observed: Observed8,
    stage: Stage8,
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn check_request8(request: &Request8, args: &[String]) -> Result<(), String> {
    if request.args_digest != digest(&serde_json::to_vec(args).map_err(|e| e.to_string())?)
        || request.source.is_empty()
        || !request
            .source
            .iter()
            .any(|source| source.digest == request.active_hash)
        || !matches!(request.target.as_str(), "gfx942" | "gfx950")
        || request
            .source
            .iter()
            .map(|s| &s.path)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != request.source.len()
    {
        return Err("lifecycle request lost exact arguments or source stamps".into());
    }
    for source in &request.source {
        if digest(&std::fs::read(&source.path).map_err(|e| e.to_string())?) != source.digest {
            return Err("lifecycle source changed during invocation".into());
        }
    }
    require_canonical_overflow_checks_v1(args)
}

#[test]
#[ignore = "subprocess helper; parent binds actual Cargo arguments and lifecycle case"]
fn fixed8_census_lifecycle_child() {
    let Some(args_path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(args_path).unwrap()).unwrap();
    let request: Request8 = serde_json::from_str(&env::var(REQUEST8).unwrap()).unwrap();
    check_request8(&request, &args).unwrap();
    assert!(!request.artifact.exists());
    let observed = Arc::new(Mutex::new(Observed8::default()));
    let callback = Arc::clone(&observed);
    let observer = Box::new(
        move |_: TyCtxt<'_>,
              semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1| {
            let mut report = callback.lock().unwrap();
            report.calls += 1;
            report.roots = Some(census::roots(semantic));
        },
    );
    let stage = Arc::new(Mutex::new(Stage8::default()));
    let stage_callback = Arc::clone(&stage);
    let input = request.input;
    let target = request.target.clone();
    let stage_observer: live::Observer = Box::new(move |view, budget| {
        let mut observed = stage_callback.lock().unwrap();
        if observed.calls != 0 {
            return Err("repeated fixed8 stage callback".into());
        }
        *observed = stage8(view, input, &target, budget)?;
        Ok(())
    });
    // The actual runner transfers both Send observers into its rustc callback.
    // It is not installed in the caller thread's semantic-observer slot.
    let result =
        crate::production_rustc_driver_v1::fixed_census_invocation_observer_v1_tests::with_observer(
            observer,
            || {
                live::with_observer(stage_observer, || {
                    rustc_driver::catch_fatal_errors(|| {
                        crate::run_production_fixed_checked_output_policy8_extraction_driver_v1(
                            &args,
                            &request.artifact,
                        )
                    })
                })
            },
        );
    let outcome = match &result {
        Ok(Ok(())) => Outcome8::Success {},
        Ok(Err(error)) => Outcome8::AdmissionFailure {
            error: error.clone(),
        },
        Err(_) => Outcome8::Fatal {},
    };
    check_request8(&request, &args).unwrap();
    let report = Report8 {
        request,
        outcome,
        observed: Arc::try_unwrap(observed)
            .expect("invocation observer dropped")
            .into_inner()
            .unwrap(),
        stage: Arc::try_unwrap(stage)
            .expect("stage observer dropped")
            .into_inner()
            .unwrap(),
    };
    use std::io::Write as _;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(env::var_os(CHILD_RESULT).unwrap())
        .unwrap()
        .write_all(&serde_json::to_vec(&report).unwrap())
        .unwrap();
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => panic!("actual public extraction refused: {error}"),
        Err(fatal) => fatal.raise(),
    }
}

fn check_outcome8(
    status: Option<i32>,
    report: &Report8,
    expected: &Request8,
) -> Result<(), String> {
    if &report.request != expected {
        return Err("foreign lifecycle report".into());
    }
    let expected_status = if expected.input.succeeds() { 0 } else { 101 };
    if status != Some(expected_status) {
        return Err("unexpected lifecycle process status".into());
    }
    if report.stage.calls != usize::from(expected.input.succeeds())
        || report.stage.erased != (expected.input == Input::RetainedUnit)
        || (expected.input.succeeds()
            && [
                report.stage.original,
                report.stage.output,
                report.stage.llvm,
            ]
            .contains(&[0; 32]))
        || (!expected.input.succeeds() && report.stage != Stage8::default())
    {
        return Err("wrong fixed8 stage route or invocation count".into());
    }
    match (&report.outcome, expected.input) {
        (Outcome8::Success {}, Input::Fill | Input::RetainedUnit) => {}
        (Outcome8::AdmissionFailure { error }, Input::WaveRefusal)
            if error.contains("production compilation pre-ranked materialization failed:")
                && error.contains(
                    "helper parameter is not an exact by-value scalar aggregate or shared slice",
                ) => {}
        (Outcome8::Fatal {}, Input::ParseFatal) => {}
        _ => return Err("wrong lifecycle outcome or admission boundary".into()),
    }
    if expected.input == Input::ParseFatal {
        if report.observed.calls != 0 || report.observed.roots.is_some() {
            return Err("parse-fatal source reached semantic import".into());
        }
    } else {
        let roots = report
            .observed
            .roots
            .as_ref()
            .ok_or("actual owner observation missing")?
            .as_ref()
            .map_err(|e| e.clone())?;
        if report.observed.calls != 1 || roots.len() != 1 || roots[0].name != expected.input.root()
        {
            return Err(
                "actual invocation did not observe exactly its selected source root".into(),
            );
        }
    }
    Ok(())
}

fn decode8(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    expected: &Request8,
) -> Result<Report8, String> {
    let report: Report8 = serde_json::from_slice(bytes.ok_or("missing fresh lifecycle report")?)
        .map_err(|e| e.to_string())?;
    check_outcome8(status, &report, expected)?;
    Ok(report)
}

struct Run8 {
    report: Report8,
    output: std::process::Output,
    bytes: Option<Vec<u8>>,
}

fn run8(case: &CapturedCase, diagnostic: Diagnostic, directory: &Path) -> Run8 {
    std::fs::create_dir(directory).unwrap();
    let artifact = directory.join("output.ll");
    let response = directory.join("result.json");
    let args_path = directory.join("args.json");
    let census_path = match diagnostic {
        Diagnostic::OutputAlias => artifact.clone(),
        Diagnostic::BindingAlias => directory.join("crate-binding.txt"),
        _ => directory.join("census.json"),
    };
    let run_id = census::run_id(&case.captured.args, directory.to_str().unwrap());
    let request = Request8 {
        input: case.input,
        target: case.target.clone(),
        diagnostic,
        args_digest: digest(&serde_json::to_vec(&case.captured.args).unwrap()),
        source: case.source.clone(),
        active_hash: case.active_hash,
        artifact: artifact.clone(),
        run_id: run_id.clone(),
    };
    check_request8(&request, &case.captured.args).unwrap();
    std::fs::write(&args_path, serde_json::to_vec(&case.captured.args).unwrap()).unwrap();
    assert!(!artifact.exists() && !response.exists() && !census_path.exists());
    if diagnostic == Diagnostic::Stale {
        std::fs::write(&census_path, STALE).unwrap();
    }
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .env_clear()
        .envs(case.captured.environment.iter().cloned())
        .current_dir(&case.captured.cwd)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove(BINDING)
        .env_remove(CHILD_PROOF_PROBE)
        .env(CHILD_ARGS, &args_path)
        .env(CHILD_RESULT, &response)
        .env(REQUEST8, serde_json::to_string(&request).unwrap())
        .args(["--exact", CHILD8, "--ignored", "--nocapture"]);
    census::configure(
        &mut command,
        match diagnostic {
            Diagnostic::Disabled => None,
            Diagnostic::InvalidRunId => Some((&census_path, "invalid-run-id")),
            _ => Some((&census_path, &run_id)),
        },
    );
    if diagnostic == Diagnostic::BindingAlias {
        command.env(BINDING, &census_path);
    }
    progress::clear_inherited_jobserver(&mut command);
    let output = command.output().unwrap();
    let response_bytes = std::fs::read(&response);
    let report = decode8(
        output.status.code(),
        response_bytes.as_deref().ok(),
        &request,
    )
    .unwrap_or_else(|e| {
        panic!(
            "fixed8/{:?}/{diagnostic:?}: {e}\n{}",
            case.input,
            corpus_cargo::diagnostics(&output)
        )
    });
    check_request8(&request, &case.captured.args).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let bytes = if case.input.succeeds() {
        let bytes = std::fs::read(&artifact).unwrap();
        assert!(!bytes.is_empty());
        let llvm = std::str::from_utf8(&bytes).unwrap();
        assert!(
            llvm.contains("amdgpu_kernel") && llvm.contains(&format!("@{}(", case.input.root()))
        );
        assert_eq!(
            stderr
                .matches("fe2o3 fixed checked-output extraction:")
                .count(),
            1
        );
        assert!(stderr.contains("fixed policy 8"));
        assert_eq!(digest(&bytes), report.stage.llvm);
        assert!(stderr.contains(if case.input == Input::RetainedUnit {
            "distinct E Some("
        } else {
            "distinct E None"
        }));
        Some(bytes)
    } else {
        assert!(!artifact.exists(), "failure emitted native output");
        if case.input == Input::ParseFatal {
            assert!(stderr.contains("unclosed delimiter"));
        }
        None
    };
    match diagnostic {
        Diagnostic::Fresh => {
            let census_report = census::read_report(&census_path).unwrap();
            census::check_header(
                &census_report,
                &case.captured.args,
                &case.captured.cwd,
                8,
                &format!("{}:xnack-", case.target),
                &run_id,
                case.input.succeeds(),
            )
            .unwrap();
            if case.input == Input::ParseFatal {
                assert_eq!(census_report["selection"]["status"], "unavailable");
                assert_eq!(
                    census_report["selection"]["value"],
                    "collection not reached"
                );
            } else {
                let roots = report.observed.roots.as_ref().unwrap().as_ref().unwrap();
                census::check_selected(&census_report, roots, &[case.active_hash]).unwrap();
            }
        }
        Diagnostic::Stale => {
            assert_eq!(std::fs::read(&census_path).unwrap(), STALE);
            let stale = census::read_report(&census_path).unwrap();
            assert!(
                census::check_header(
                    &stale,
                    &case.captured.args,
                    &case.captured.cwd,
                    8,
                    &format!("{}:xnack-", case.target),
                    &run_id,
                    case.input.succeeds()
                )
                .is_err()
            );
        }
        Diagnostic::OutputAlias => assert_eq!(std::fs::read(&census_path).ok(), bytes),
        Diagnostic::Disabled | Diagnostic::InvalidRunId | Diagnostic::BindingAlias => {
            assert!(!census_path.exists())
        }
    }
    if !matches!(diagnostic, Diagnostic::Disabled | Diagnostic::Fresh) {
        assert!(stderr.contains("fe2o3 diagnostic source census unavailable:"));
    }
    Run8 {
        report,
        output,
        bytes,
    }
}

fn unchanged8(baseline: &Run8, candidate: &Run8) {
    assert_eq!(baseline.report.stage, candidate.report.stage);
    assert_eq!(
        baseline.output.status, candidate.output.status,
        "diagnostics changed actual process status"
    );
    assert_eq!(
        baseline.report.outcome, candidate.report.outcome,
        "diagnostics changed actual public result"
    );
    assert_eq!(
        baseline.report.observed, candidate.report.observed,
        "diagnostics changed actual source owner observation"
    );
    assert_eq!(
        baseline.bytes, candidate.bytes,
        "diagnostics changed actual native bytes"
    );
}

#[test]
#[ignore = "real fixed8 public runner, pinned nightly and AMD dependencies; 26 compiler children"]
fn fixed8_source_routes_and_census_lifecycles_are_observational() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-fixed8-census-lifecycle");
    let mut direct_gfx942 = None;
    let mut completed = 0;
    for target in ["gfx942", "gfx950"] {
        for input in [Input::Fill, Input::RetainedUnit] {
            let case = capture(&workspace, scratch.path(), input, target);
            let disabled = run8(
                &case,
                Diagnostic::Disabled,
                &scratch
                    .path()
                    .join(format!("route-{target}-{input:?}-disabled")),
            );
            let enabled = run8(
                &case,
                Diagnostic::Fresh,
                &scratch
                    .path()
                    .join(format!("route-{target}-{input:?}-fresh")),
            );
            unchanged8(&disabled, &enabled);
            for run in [&disabled, &enabled] {
                let stderr = String::from_utf8_lossy(&run.output.stderr);
                assert!(
                    stderr.contains("historical I ")
                        && stderr.contains("historical J ")
                        && stderr.contains("actual K ")
                );
                assert!(!stderr.contains("actual I "));
            }
            completed += 2;
            if target == "gfx942" && input == Input::Fill {
                direct_gfx942 = Some(case);
            }
        }
    }
    let direct = direct_gfx942.unwrap();
    let refusal = capture(&workspace, scratch.path(), Input::WaveRefusal, "gfx942");
    let fatal = parse_fatal(&workspace, scratch.path(), &direct);
    for case in [&direct, &refusal, &fatal] {
        let mut baseline = None;
        for diagnostic in Diagnostic::ALL {
            let result = run8(
                case,
                diagnostic,
                &scratch
                    .path()
                    .join(format!("lifecycle-{:?}-{diagnostic:?}", case.input)),
            );
            if let Some(baseline) = &baseline {
                unchanged8(baseline, &result);
            } else {
                baseline = Some(result);
            }
            completed += 1;
        }
    }
    assert_eq!(completed, 26);
    eprintln!(
        "FIXED8 CENSUS: 4 source/route cases and 18 lifecycle controls; 26 actual public invocations; no source UnitLocal mutation claim"
    );
}
#[test]
fn lifecycle_protocol_rejects_wrong_status_owner_count_outcome_and_foreign_request() {
    let request = Request8 {
        input: Input::Fill,
        target: "gfx942".into(),
        diagnostic: Diagnostic::Fresh,
        args_digest: [1; 32],
        source: vec![],
        active_hash: [2; 32],
        artifact: "unused".into(),
        run_id: "current".into(),
    };
    let exact = |request: Request8, calls, outcome| Report8 {
        stage: Stage8 {
            calls: 1,
            original: [5; 32],
            output: [6; 32],
            llvm: [7; 32],
            ..Stage8::default()
        },
        request,
        outcome,
        observed: Observed8 {
            calls,
            roots: Some(Ok(vec![census::SourceRoot {
                name: "fill".into(),
                function: [3; 32],
                body: [4; 32],
            }])),
        },
    };
    let good = serde_json::to_vec(&exact(request.clone(), 1, Outcome8::Success {})).unwrap();
    decode8(Some(0), Some(&good), &request).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&good).unwrap();
    for pointer in ["/outcome", "/observed", "/stage", "/request"] {
        let mut changed = value.clone();
        changed
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), true.into());
        assert!(
            decode8(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                &request
            )
            .is_err()
        );
    }
    for change in [0, 1, 2, 3] {
        let mut changed = value.clone();
        match change {
            0 => changed["outcome"]["kind"] = "UnexpectedOutcome".into(),
            1 => changed["stage"]["calls"] = 2.into(),
            2 => changed["stage"]["erased"] = true.into(),
            3 => changed["stage"]["llvm"] = serde_json::json!(([0_u8; 32])),
            _ => unreachable!(),
        }
        assert!(
            decode8(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                &request
            )
            .is_err()
        );
    }
    for status in [None, Some(1), Some(101), Some(134), Some(137)] {
        assert!(decode8(status, Some(&good), &request).is_err());
    }
    for bytes in [None, Some(&b"{}"[..]), Some(&b"not-json"[..])] {
        assert!(decode8(Some(0), bytes, &request).is_err());
    }
    for (calls, outcome) in [
        (0, Outcome8::Success {}),
        (2, Outcome8::Success {}),
        (1, Outcome8::Fatal {}),
        (
            1,
            Outcome8::AdmissionFailure {
                error: "wrong stage".into(),
            },
        ),
    ] {
        let bytes = serde_json::to_vec(&exact(request.clone(), calls, outcome)).unwrap();
        assert!(decode8(Some(0), Some(&bytes), &request).is_err());
    }
    let mut foreign = request.clone();
    foreign.target = "gfx950".into();
    assert!(decode8(Some(0), Some(&good), &foreign).is_err());
    foreign = request.clone();
    foreign.args_digest[0] ^= 1;
    assert!(decode8(Some(0), Some(&good), &foreign).is_err());
    foreign = request;
    foreign.run_id = "stale".into();
    assert!(decode8(Some(0), Some(&good), &foreign).is_err());
}

#[test]
fn lifecycle_failure_protocol_distinguishes_admission_fatal_and_callback_panics() {
    let mut request = Request8 {
        input: Input::WaveRefusal,
        target: "gfx942".into(),
        diagnostic: Diagnostic::Fresh,
        args_digest: [1; 32],
        source: vec![],
        active_hash: [2; 32],
        artifact: "unused".into(),
        run_id: "current".into(),
    };
    let refusal = "production compilation pre-ranked materialization failed: helper parameter is not an exact by-value scalar aggregate or shared slice";
    let mut report = Report8 {
        stage: Stage8::default(),
        request: request.clone(),
        outcome: Outcome8::AdmissionFailure {
            error: refusal.into(),
        },
        observed: Observed8 {
            calls: 1,
            roots: Some(Ok(vec![census::SourceRoot {
                name: "wave64_capture".into(),
                function: [3; 32],
                body: [4; 32],
            }])),
        },
    };
    let encode = |report: &Report8| serde_json::to_vec(report).unwrap();
    decode8(Some(101), Some(&encode(&report)), &request).unwrap();
    assert!(decode8(Some(0), Some(&encode(&report)), &request).is_err());
    for error in [
        "different source refusal",
        "rustc or callback panicked",
        "diagnostic setup failed",
    ] {
        report.outcome = Outcome8::AdmissionFailure {
            error: error.into(),
        };
        assert!(decode8(Some(101), Some(&encode(&report)), &request).is_err());
    }
    request.input = Input::ParseFatal;
    report.request = request.clone();
    report.outcome = Outcome8::Fatal {};
    assert!(decode8(Some(101), Some(&encode(&report)), &request).is_err());
    report.observed = Observed8::default();
    decode8(Some(101), Some(&encode(&report)), &request).unwrap();
    for status in [None, Some(0), Some(1), Some(134), Some(137)] {
        assert!(decode8(status, Some(&encode(&report)), &request).is_err());
    }
}
