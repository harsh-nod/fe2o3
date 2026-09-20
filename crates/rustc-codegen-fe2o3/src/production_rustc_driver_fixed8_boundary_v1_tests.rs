//! Four unsigned native-proof controls and two real output-refusal controls.
use super::*;
use crate::production_rustc_driver_v1::fixed_census_invocation_observer_v1_tests as invocation;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::fixed_census_lifecycle::policy8::boundary::fixed8_boundary_child";
const REQUEST: &str = "FE2O3_TEST_FIXED8_BOUNDARY_REQUEST_V1";
const SENTINEL: &[u8] = b"existing-output-must-not-change\n";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Mode {
    MissingProof {},
    ExistingOutput {},
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    source: Request8,
    mode: Mode,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    observed: Observed8,
    stage: Stage8,
    checked: bool,
}

struct ProofCallbacks {
    input: Input,
    target: String,
    observer: Option<invocation::Observer>,
    result: Option<Result<(), String>>,
}
impl Callbacks for ProofCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(invocation::in_callback(self.observer.take(), || {
            let ranked = transaction_with_census_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                None,
            )?
            .verify_general_kernel_checks()
            .map_err(|e| e.to_string())?;
            let mut work = Work::new(
                usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                    .map_err(|_| "work limit")?,
            );
            let mut budget = Budget::new(
                &mut work,
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            let ledger = budget.work_ledger_identity_v1();
            live::require_missing_signature(
                ranked,
                self.input == Input::RetainedUnit,
                &self.target,
                &mut budget,
            )?;
            if budget.storage() != 0
                || budget.work() == 0
                || budget.work_ledger_identity_v1() != ledger
            {
                return Err("proof refusal lost original budget/floor".into());
            }
            Ok(())
        }));
        Compilation::Stop
    }
}

fn check(status: Option<i32>, bytes: Option<&[u8]>, expected: &Request) -> Result<Report, String> {
    let report: Report = serde_json::from_slice(bytes.ok_or("missing fresh boundary report")?)
        .map_err(|e| e.to_string())?;
    if status != Some(0)
        || &report.request != expected
        || !report.checked
        || report.observed.calls != 1
    {
        return Err("boundary status/request/actual source count".into());
    }
    let roots = report
        .observed
        .roots
        .as_ref()
        .ok_or("missing source roots")?
        .as_ref()
        .map_err(|e| e.clone())?;
    if roots.len() != 1 || roots[0].name != expected.source.input.root() {
        return Err("boundary source root mismatch".into());
    }
    match expected.mode {
        Mode::MissingProof {} if report.stage == Stage8::default() => {}
        Mode::ExistingOutput {}
            if expected.source.input == Input::Fill
                && report.stage.calls == 1
                && !report.stage.erased => {}
        _ => return Err("boundary mode/stage mismatch".into()),
    }
    Ok(report)
}

#[test]
#[ignore = "subprocess helper; four proof and two output controls have separate parent denominators"]
fn fixed8_boundary_child() {
    let Some(args_path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(args_path).unwrap()).unwrap();
    let request: Request = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    check_request8(&request.source, &args).unwrap();
    assert!(request.source.input.succeeds());
    assert_eq!(request.source.diagnostic, Diagnostic::Disabled);
    let observed = Arc::new(Mutex::new(Observed8::default()));
    let callback = Arc::clone(&observed);
    let observer = Box::new(
        move |_: TyCtxt<'_>,
              semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1| {
            let mut observed = callback.lock().unwrap();
            observed.calls += 1;
            observed.roots = Some(census::roots(semantic));
        },
    );
    let stage = Arc::new(Mutex::new(Stage8::default()));
    invocation::with_observer(observer, || match request.mode {
        Mode::MissingProof {} => {
            assert!(!request.source.artifact.exists());
            let mut callbacks = ProofCallbacks {
                input: request.source.input,
                target: request.source.target.clone(),
                observer: invocation::take_for_invocation(),
                result: None,
            };
            rustc_driver::run_compiler(&args, &mut callbacks);
            callbacks
                .result
                .expect("actual proof callback reached")
                .unwrap();
            assert!(!request.source.artifact.exists());
        }
        Mode::ExistingOutput {} => {
            assert_eq!(std::fs::read(&request.source.artifact).unwrap(), SENTINEL);
            let callback = Arc::clone(&stage);
            let target = request.source.target.clone();
            let observer: live::Observer = Box::new(move |view, budget| {
                let mut observed = callback.lock().unwrap();
                if observed.calls != 0 {
                    return Err("duplicate output-refusal stage".into());
                }
                *observed = stage8(view, Input::Fill, &target, budget)?;
                Ok(())
            });
            let error = live::with_observer(observer, || {
                crate::run_production_fixed_checked_output_policy8_extraction_driver_v1(
                    &args,
                    &request.source.artifact,
                )
            })
            .unwrap_err();
            assert!(
                error.contains("failed to create new fixed checked-output LLVM extraction output"),
                "{error}"
            );
            assert_eq!(std::fs::read(&request.source.artifact).unwrap(), SENTINEL);
        }
    });
    check_request8(&request.source, &args).unwrap();
    let report = Report {
        request,
        observed: Arc::try_unwrap(observed)
            .expect("semantic observer dropped")
            .into_inner()
            .unwrap(),
        stage: Arc::try_unwrap(stage)
            .expect("stage observer dropped")
            .into_inner()
            .unwrap(),
        checked: true,
    };
    let bytes = serde_json::to_vec(&report).unwrap();
    check(Some(0), Some(&bytes), &report.request).unwrap();
    use std::io::Write as _;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(env::var_os(CHILD_RESULT).unwrap())
        .unwrap()
        .write_all(&bytes)
        .unwrap();
}

fn run_boundary(case: &CapturedCase, mode: Mode, directory: &Path) {
    std::fs::create_dir(directory).unwrap();
    let artifact = directory.join("output.ll");
    let response = directory.join("response.json");
    let args_path = directory.join("args.json");
    let request = Request {
        source: Request8 {
            input: case.input,
            target: case.target.clone(),
            diagnostic: Diagnostic::Disabled,
            args_digest: digest(&serde_json::to_vec(&case.captured.args).unwrap()),
            source: case.source.clone(),
            active_hash: case.active_hash,
            artifact: artifact.clone(),
            run_id: census::run_id(&case.captured.args, directory.to_str().unwrap()),
        },
        mode,
    };
    check_request8(&request.source, &case.captured.args).unwrap();
    std::fs::write(&args_path, serde_json::to_vec(&case.captured.args).unwrap()).unwrap();
    assert!(!artifact.exists() && !response.exists());
    if matches!(request.mode, Mode::ExistingOutput {}) {
        std::fs::write(&artifact, SENTINEL).unwrap();
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
    census::configure(&mut command, None);
    progress::clear_inherited_jobserver(&mut command);
    let output = command.output().unwrap();
    check(
        output.status.code(),
        std::fs::read(&response).as_deref().ok(),
        &request,
    )
    .unwrap_or_else(|e| panic!("{e}\n{}", corpus_cargo::diagnostics(&output)));
    check_request8(&request.source, &case.captured.args).unwrap();
    match request.mode {
        Mode::MissingProof {} => assert!(!artifact.exists()),
        Mode::ExistingOutput {} => assert_eq!(std::fs::read(artifact).unwrap(), SENTINEL),
    }
}

#[test]
#[ignore = "actual unsigned Direct/UnitLocal native admission on both targets; four compiler children"]
fn fixed8_actual_unsigned_sources_require_signed_native_proof() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fixed8-proof-controls");
    let mut count = 0;
    for target in ["gfx942", "gfx950"] {
        for input in [Input::Fill, Input::RetainedUnit] {
            let case = capture(&workspace, scratch.path(), input, target);
            run_boundary(
                &case,
                Mode::MissingProof {},
                &scratch.path().join(format!("proof-{target}-{input:?}")),
            );
            count += 1;
        }
    }
    assert_eq!(count, 4);
}

#[test]
#[ignore = "actual public fixed8 output failure on both targets; two compiler children"]
fn fixed8_public_output_refusal_preserves_existing_bytes() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fixed8-output-controls");
    let mut count = 0;
    for target in ["gfx942", "gfx950"] {
        let case = capture(&workspace, scratch.path(), Input::Fill, target);
        run_boundary(
            &case,
            Mode::ExistingOutput {},
            &scratch.path().join(format!("output-{target}")),
        );
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn boundary_mode_is_closed_including_nested_unit_fields() {
    for json in [
        r#"{"kind":"MissingProof","extra":true}"#,
        r#"{"kind":"ExistingOutput","policy":7}"#,
        r#"{"kind":"SignedSuccess"}"#,
    ] {
        assert!(serde_json::from_str::<Mode>(json).is_err());
    }
    for mode in [Mode::MissingProof {}, Mode::ExistingOutput {}] {
        assert_eq!(
            serde_json::from_slice::<Mode>(&serde_json::to_vec(&mode).unwrap()).unwrap(),
            mode
        );
    }
}
