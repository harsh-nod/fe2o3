//! Real public-runner controls; diagnostic failures never qualify source output.
use super::fixed_census_observation as census;
use super::*;
use std::sync::{Arc, Mutex};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::fixed_census_lifecycle::fixed_census_lifecycle_child";
const REQUEST: &str = "FE2O3_TEST_FIXED_CENSUS_LIFECYCLE_REQUEST_V1";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const BINDING: &str = "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1";
const STALE: &[u8] = b"{\"runId\":\"stale-sentinel\",\"extractionSucceeded\":true}\n";

#[path = "production_rustc_driver_fixed7_census_lifecycle_v1_tests.rs"]
mod policy7;
#[path = "production_rustc_driver_fixed8_census_lifecycle_v1_tests.rs"]
mod policy8;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Policy {
    Five,
    Six,
    Seven,
}
impl Policy {
    fn number(self) -> u16 {
        match self {
            Self::Five => 5,
            Self::Six => 6,
            Self::Seven => 7,
        }
    }
    fn run(self, args: &[String], output: &Path) -> Result<(), String> {
        match self {
            Self::Five => {
                crate::run_production_fixed_checked_output_extraction_driver_v1(args, output)
            }
            Self::Six => crate::run_production_fixed_checked_output_policy6_extraction_driver_v1(
                args, output,
            ),
            Self::Seven => crate::run_production_fixed_checked_output_policy7_extraction_driver_v1(
                args, output,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Input {
    Fill,
    RetainedUnit,
    WaveRefusal,
    ParseFatal,
}
impl Input {
    fn root(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::RetainedUnit => "private_helper_fill",
            Self::WaveRefusal => "wave64_capture",
            Self::ParseFatal => "no-root",
        }
    }
    fn active_source(self) -> &'static str {
        match self {
            Self::WaveRefusal => "src/wave64_capture.rs",
            _ => "src/lib.rs",
        }
    }
    fn succeeds(self) -> bool {
        matches!(self, Self::Fill | Self::RetainedUnit)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Diagnostic {
    Disabled,
    Fresh,
    InvalidRunId,
    Stale,
    OutputAlias,
    BindingAlias,
}
impl Diagnostic {
    const ALL: [Self; 6] = [
        Self::Disabled,
        Self::Fresh,
        Self::InvalidRunId,
        Self::Stale,
        Self::OutputAlias,
        Self::BindingAlias,
    ];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Stamp {
    path: PathBuf,
    digest: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    policy: Policy,
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
enum Outcome {
    Success,
    AdmissionFailure(String),
    Fatal,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct Observed {
    calls: usize,
    roots: Option<Result<Vec<census::SourceRoot>, String>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    outcome: Outcome,
    observed: Observed,
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn stamp(path: PathBuf) -> Stamp {
    let path = path.canonicalize().unwrap();
    Stamp {
        digest: digest(&std::fs::read(&path).unwrap()),
        path,
    }
}

fn check_request(request: &Request, args: &[String]) -> Result<(), String> {
    if request.args_digest != digest(&serde_json::to_vec(args).map_err(|e| e.to_string())?)
        || request.source.is_empty()
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
fn fixed_census_lifecycle_child() {
    let Some(args_path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(args_path).unwrap()).unwrap();
    let request: Request = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    check_request(&request, &args).unwrap();
    assert!(!request.artifact.exists());
    let observed = Arc::new(Mutex::new(Observed::default()));
    let callback = Arc::clone(&observed);
    let observer = Box::new(
        move |_: TyCtxt<'_>,
              semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1| {
            let mut report = callback.lock().unwrap();
            report.calls += 1;
            report.roots = Some(census::roots(semantic));
        },
    );
    // The actual runner transfers this Send observer into its rustc callback.
    // It is not installed in the caller thread's semantic-observer slot.
    let result =
        super::super::fixed_census_invocation_observer_v1_tests::with_observer(observer, || {
            rustc_driver::catch_fatal_errors(|| request.policy.run(&args, &request.artifact))
        });
    let outcome = match &result {
        Ok(Ok(())) => Outcome::Success,
        Ok(Err(error)) => Outcome::AdmissionFailure(error.clone()),
        Err(_) => Outcome::Fatal,
    };
    check_request(&request, &args).unwrap();
    let report = Report {
        request,
        outcome,
        observed: Arc::try_unwrap(observed)
            .expect("invocation observer dropped")
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

fn check_outcome(status: Option<i32>, report: &Report, expected: &Request) -> Result<(), String> {
    if &report.request != expected {
        return Err("foreign lifecycle report".into());
    }
    let expected_status = if expected.input.succeeds() { 0 } else { 101 };
    if status != Some(expected_status) {
        return Err("unexpected lifecycle process status".into());
    }
    match (&report.outcome, expected.input) {
        (Outcome::Success, Input::Fill | Input::RetainedUnit) => {}
        (Outcome::AdmissionFailure(error), Input::WaveRefusal)
            if error.contains("production compilation pre-ranked materialization failed:")
                && error.contains(
                    "helper parameter is not an exact by-value scalar aggregate or shared slice",
                ) => {}
        (Outcome::Fatal, Input::ParseFatal) => {}
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

fn decode(status: Option<i32>, bytes: Option<&[u8]>, expected: &Request) -> Result<Report, String> {
    let report: Report = serde_json::from_slice(bytes.ok_or("missing fresh lifecycle report")?)
        .map_err(|e| e.to_string())?;
    check_outcome(status, &report, expected)?;
    Ok(report)
}

struct CapturedCase {
    captured: corpus_cargo::Captured,
    input: Input,
    target: String,
    source: Vec<Stamp>,
    active_hash: [u8; 32],
}

fn capture(workspace: &Path, scratch: &Path, input: Input, target: &str) -> CapturedCase {
    assert_ne!(input, Input::ParseFatal);
    let relative = format!("{BASE}/{}", input.active_source());
    let mut source_paths = vec![format!("{BASE}/src/lib.rs")];
    if relative != source_paths[0] {
        source_paths.push(relative.clone());
    }
    let source = std::iter::once("Cargo.lock".to_owned())
        .chain(std::iter::once(format!("{BASE}/Cargo.toml")))
        .chain(source_paths.iter().cloned())
        .map(|path| stamp(workspace.join(path)))
        .collect::<Vec<_>>();
    let hash = |path: &str| {
        digest(&std::fs::read(workspace.join(path)).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let manifest = format!("{BASE}/Cargo.toml");
    let fixture = corpus::Fixture {
        fixture_id: format!("census-{target}-{input:?}"),
        target: target.into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths,
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: match input {
                Input::Fill => vec![],
                Input::RetainedUnit => vec!["private-unit-helper".into()],
                Input::WaveRefusal => vec!["wave64-capture-u32".into()],
                Input::ParseFatal => unreachable!(),
            },
            kernel_symbols: vec![input.root().into()],
        },
    };
    let directory = scratch.join(&fixture.fixture_id);
    std::fs::create_dir(&directory).unwrap();
    let mut captured =
        corpus_cargo::capture(workspace, &fixture, &directory, &scratch.join(target)).unwrap();
    require_canonical_overflow_checks_v1(&captured.args).unwrap();
    if input == Input::RetainedUnit {
        captured
            .args
            .extend(["-Zinline-mir=no".into(), "-Zmir-opt-level=0".into()]);
    }
    CapturedCase {
        captured,
        input,
        target: target.into(),
        source,
        active_hash: digest(&std::fs::read(workspace.join(relative)).unwrap()),
    }
}

fn parse_fatal(workspace: &Path, scratch: &Path, original: &CapturedCase) -> CapturedCase {
    let path = scratch.join("broken.rs");
    std::fs::write(&path, "#![no_std]\npub fn broken( {\n").unwrap();
    let original_input = workspace
        .join(format!("{BASE}/src/lib.rs"))
        .canonicalize()
        .unwrap();
    let mut args = original.captured.args.clone();
    let inputs = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| !arg.starts_with('-'))
        .filter(|(_, arg)| {
            original.captured.cwd.join(arg).canonicalize().ok().as_ref() == Some(&original_input)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [input] = inputs.as_slice() else {
        panic!("exact captured root input missing or duplicated")
    };
    args[*input] = path.to_str().unwrap().into();
    let broken = stamp(path);
    CapturedCase {
        captured: corpus_cargo::Captured {
            args,
            environment: original.captured.environment.clone(),
            cwd: original.captured.cwd.clone(),
            cfg: original.captured.cfg.clone(),
            cargo_diagnostics: original.captured.cargo_diagnostics.clone(),
        },
        input: Input::ParseFatal,
        target: original.target.clone(),
        active_hash: broken.digest,
        source: vec![broken],
    }
}

struct Run {
    report: Report,
    output: std::process::Output,
    bytes: Option<Vec<u8>>,
}

fn run(case: &CapturedCase, policy: Policy, diagnostic: Diagnostic, directory: &Path) -> Run {
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
    let request = Request {
        policy,
        input: case.input,
        target: case.target.clone(),
        diagnostic,
        args_digest: digest(&serde_json::to_vec(&case.captured.args).unwrap()),
        source: case.source.clone(),
        active_hash: case.active_hash,
        artifact: artifact.clone(),
        run_id: run_id.clone(),
    };
    check_request(&request, &case.captured.args).unwrap();
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
        .env(REQUEST, serde_json::to_string(&request).unwrap())
        .args(["--exact", CHILD, "--ignored", "--nocapture"]);
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
    let report = decode(
        output.status.code(),
        response_bytes.as_deref().ok(),
        &request,
    )
    .unwrap_or_else(|e| {
        panic!(
            "{policy:?}/{:?}/{diagnostic:?}: {e}\n{}",
            case.input,
            corpus_cargo::diagnostics(&output)
        )
    });
    check_request(&request, &case.captured.args).unwrap();
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
        assert!(stderr.contains(&format!("fixed policy {}", policy.number())));
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
                policy.number(),
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
                    policy.number(),
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
    Run {
        report,
        output,
        bytes,
    }
}

fn unchanged(baseline: &Run, candidate: &Run) {
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
#[ignore = "real fixed public runners, pinned nightly and AMD dependencies; 44 compiler children"]
fn fixed5_source_routes_and_fixed5_fixed6_census_lifecycles_are_observational() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-fixed-census-lifecycle");
    let mut direct_gfx942 = None;
    let mut completed = 0;
    for target in ["gfx942", "gfx950"] {
        for input in [Input::Fill, Input::RetainedUnit] {
            let case = capture(&workspace, scratch.path(), input, target);
            let baseline = run(
                &case,
                Policy::Five,
                Diagnostic::Disabled,
                &scratch
                    .path()
                    .join(format!("positive-{target}-{input:?}-disabled")),
            );
            let enabled = run(
                &case,
                Policy::Five,
                Diagnostic::Fresh,
                &scratch
                    .path()
                    .join(format!("positive-{target}-{input:?}-fresh")),
            );
            unchanged(&baseline, &enabled);
            completed += 2;
            if target == "gfx942" && input == Input::Fill {
                direct_gfx942 = Some(case);
            }
        }
    }
    let direct = direct_gfx942.unwrap();
    let refusal = capture(&workspace, scratch.path(), Input::WaveRefusal, "gfx942");
    let fatal = parse_fatal(&workspace, scratch.path(), &direct);
    for policy in [Policy::Five, Policy::Six] {
        for case in [&direct, &refusal, &fatal] {
            let mut baseline = None;
            for diagnostic in Diagnostic::ALL {
                let result = run(
                    case,
                    policy,
                    diagnostic,
                    &scratch.path().join(format!(
                        "lifecycle-{policy:?}-{:?}-{diagnostic:?}",
                        case.input
                    )),
                );
                if let Some(baseline) = &baseline {
                    unchanged(baseline, &result);
                } else {
                    baseline = Some(result);
                }
                completed += 1;
            }
        }
    }
    assert_eq!(completed, 44);
    eprintln!(
        "FIXED CENSUS: 4 fixed5 source/route cases and 36 lifecycle controls; 44 actual public invocations"
    );
}

#[test]
fn lifecycle_protocol_rejects_wrong_status_owner_count_outcome_and_foreign_request() {
    let request = Request {
        policy: Policy::Five,
        input: Input::Fill,
        target: "gfx942".into(),
        diagnostic: Diagnostic::Fresh,
        args_digest: [1; 32],
        source: vec![],
        active_hash: [2; 32],
        artifact: "unused".into(),
        run_id: "current".into(),
    };
    let exact = |request: Request, calls, outcome| Report {
        request,
        outcome,
        observed: Observed {
            calls,
            roots: Some(Ok(vec![census::SourceRoot {
                name: "fill".into(),
                function: [3; 32],
                body: [4; 32],
            }])),
        },
    };
    let good = serde_json::to_vec(&exact(request.clone(), 1, Outcome::Success)).unwrap();
    decode(Some(0), Some(&good), &request).unwrap();
    for status in [None, Some(1), Some(101), Some(134), Some(137)] {
        assert!(decode(status, Some(&good), &request).is_err());
    }
    for bytes in [None, Some(&b"{}"[..]), Some(&b"not-json"[..])] {
        assert!(decode(Some(0), bytes, &request).is_err());
    }
    for (calls, outcome) in [
        (0, Outcome::Success),
        (2, Outcome::Success),
        (1, Outcome::Fatal),
        (1, Outcome::AdmissionFailure("wrong stage".into())),
    ] {
        let bytes = serde_json::to_vec(&exact(request.clone(), calls, outcome)).unwrap();
        assert!(decode(Some(0), Some(&bytes), &request).is_err());
    }
    let mut foreign = request.clone();
    foreign.policy = Policy::Six;
    assert!(decode(Some(0), Some(&good), &foreign).is_err());
    foreign = request.clone();
    foreign.args_digest[0] ^= 1;
    assert!(decode(Some(0), Some(&good), &foreign).is_err());
    foreign = request;
    foreign.run_id = "stale".into();
    assert!(decode(Some(0), Some(&good), &foreign).is_err());
}

#[test]
fn lifecycle_failure_protocol_distinguishes_admission_fatal_and_callback_panics() {
    let mut request = Request {
        policy: Policy::Six,
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
    let mut report = Report {
        request: request.clone(),
        outcome: Outcome::AdmissionFailure(refusal.into()),
        observed: Observed {
            calls: 1,
            roots: Some(Ok(vec![census::SourceRoot {
                name: "wave64_capture".into(),
                function: [3; 32],
                body: [4; 32],
            }])),
        },
    };
    let encode = |report: &Report| serde_json::to_vec(report).unwrap();
    decode(Some(101), Some(&encode(&report)), &request).unwrap();
    assert!(decode(Some(0), Some(&encode(&report)), &request).is_err());
    for error in [
        "different source refusal",
        "rustc or callback panicked",
        "diagnostic setup failed",
    ] {
        report.outcome = Outcome::AdmissionFailure(error.into());
        assert!(decode(Some(101), Some(&encode(&report)), &request).is_err());
    }
    request.input = Input::ParseFatal;
    report.request = request.clone();
    report.outcome = Outcome::Fatal;
    assert!(decode(Some(101), Some(&encode(&report)), &request).is_err());
    report.observed = Observed::default();
    decode(Some(101), Some(&encode(&report)), &request).unwrap();
    for status in [None, Some(0), Some(1), Some(134), Some(137)] {
        assert!(decode(status, Some(&encode(&report)), &request).is_err());
    }
}
