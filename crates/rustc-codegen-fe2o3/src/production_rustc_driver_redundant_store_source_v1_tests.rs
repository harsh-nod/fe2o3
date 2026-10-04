//! Shared strict source protocol for the original smoke and separate fixed-width matrix.
use super::fixed_census_observation as census;
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::source_observation;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::sync::{Arc, Mutex};

#[path = "production_rustc_driver_redundant_store_active_source_v1_tests.rs"]
mod active_source;
#[path = "production_rustc_driver_redundant_store_graph_v1_tests.rs"]
mod graph;
#[path = "production_rustc_driver_redundant_store_matrix_v1_tests.rs"]
mod matrix;
#[path = "production_rustc_driver_redundant_store_simulation_v1_tests.rs"]
mod sim;
use matrix::{Case, Integer, Target};

const ROOT: &str = "private_store_policy7";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const REQUEST: &str = "FE2O3_TEST_POLICY7_SMOKE_REQUEST_V1";
const ARTIFACT: &str = "FE2O3_TEST_POLICY7_SMOKE_ARTIFACT_V1";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::redundant_store_source::policy7_redundant_store_source_child";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Mode {
    Observe,
    Extract,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Stamp {
    path: PathBuf,
    sha256: [u8; 32],
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    case: Case,
    mode: Mode,
    args_sha256: [u8; 32],
    source: Vec<Stamp>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    result: Result<Outcome, String>,
}
#[derive(Debug, Deserialize, Serialize)]
enum Outcome {
    Observed(Box<graph::Observed>),
    Extracted {
        source: Source,
        llvm_sha256: [u8; 32],
        llvm_bytes: usize,
    },
}
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Source {
    semantic: [u8; 32],
    roots: Vec<census::SourceRoot>,
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn stamps(case: Case) -> Vec<Stamp> {
    [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/{}", case.fixture()),
    ]
    .into_iter()
    .map(|p: String| {
        let path = workspace().join(p).canonicalize().unwrap();
        Stamp {
            sha256: digest(&std::fs::read(&path).unwrap()),
            path,
        }
    })
    .collect()
}
fn check_request(request: &Request, args: &[String]) -> Result<(), String> {
    if request.source != stamps(request.case)
        || request.args_sha256 != digest(&serde_json::to_vec(args).map_err(|e| e.to_string())?)
        || args
            .iter()
            .filter(|arg| arg.starts_with("-Zmir-opt-level"))
            .map(String::as_str)
            .collect::<Vec<_>>()
            != ["-Zmir-opt-level=0"]
        || args
            .iter()
            .filter(|arg| arg.starts_with("-Zinline-mir"))
            .map(String::as_str)
            .collect::<Vec<_>>()
            != ["-Zinline-mir=no"]
    {
        return Err("Policy7 smoke requires exact current source/argv and opt0 retention".into());
    }
    require_canonical_overflow_checks_v1(args)
}
fn source(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    case: Case,
) -> Result<Source, String> {
    let roots = census::roots(semantic)?;
    if roots.len() != 1 || roots[0].name != case.root() {
        return Err("Policy7 smoke requires exactly its actual source root".into());
    }
    let identity = *semantic.semantic_sha256().as_bytes();
    if digest(semantic.canonical_encoding()) != identity {
        return Err("actual admitted semantic bytes/identity disagree".into());
    }
    Ok(Source {
        semantic: identity,
        roots,
    })
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| e.to_string())
}

fn observe(tcx: TyCtxt<'_>, request: &Request, artifact: &Path) -> Result<Outcome, String> {
    let case = request.case;
    let active_source = active_source::resolve(
        tcx,
        request.source.last().ok_or("missing active source stamp")?,
    )?;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| format!("sole source transaction: {e}"))?;
    let ranked = transaction
        .verify_general_kernel_checks()
        .map_err(|e| format!("ranked: {e:?}"))?;
    if !ranked.all_kernel_checks_are_clean() || ranked.grants_artifact_or_launch_authority() {
        return Err("ranked source checks/authority changed".into());
    }
    let observed = Arc::new(Mutex::new((0_usize, None)));
    let state = Arc::clone(&observed);
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let mut emitted = None;
    source_observation::with_observer(
        Box::new(move |view, budget| {
            let mut state = state.lock().unwrap();
            state.0 += 1;
            if state.0 != 1 {
                return Err("second actual Stage7 observation".into());
            }
            state.1 = Some(graph::observe(view, active_source, case, budget)?);
            Ok(())
        }),
        || {
            ranked.with_fixed_checked_output_policy7_extraction_v1(
                &mut budget,
                |identity, handoff, descriptor| {
                    if identity.policy != 7
                        || identity.erased.is_some()
                        || identity.input == identity.output
                        || handoff.target() != case.target().device()
                        || handoff.code_object_version()
                            != fe2o3_compiler_ffi::CodeObjectVersion::V6
                        || descriptor.grants_link_authority()
                        || descriptor.grants_load_authority()
                        || descriptor.grants_launch_authority()
                    {
                        return Err(
                            "actual J extraction identity, target or authority changed".into()
                        );
                    }
                    write_new(artifact, handoff.module_bytes())?;
                    emitted = Some((
                        identity.original,
                        identity.input,
                        identity.output,
                        digest(handoff.module_bytes()),
                        handoff.module_bytes().len(),
                    ));
                    Ok(())
                },
            )
        },
    )
    .map_err(|e| format!("fixed Policy7: {e:?}"))??;
    if budget.storage() != 0 {
        return Err("Policy7 extraction lost entry storage floor".into());
    }
    let (count, report) = Arc::try_unwrap(observed)
        .expect("observer dropped")
        .into_inner()
        .unwrap();
    let report = report.ok_or("actual Stage7 was not observed")?;
    if count != 1
        || emitted
            != Some((
                report.original,
                report.input,
                report.output,
                report.llvm_sha256,
                report.llvm_bytes,
            ))
    {
        return Err("observed actual owner differs from consumed J extraction".into());
    }
    graph::validate(&report, case)?;
    Ok(Outcome::Observed(Box::new(report)))
}
struct SmokeCallbacks {
    request: Request,
    artifact: PathBuf,
    result: Option<Result<Outcome, String>>,
}
impl Callbacks for SmokeCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(observe(tcx, &self.request, &self.artifact));
        Compilation::Stop
    }
}
fn extract(args: &[String], artifact: &Path, case: Case) -> Result<Outcome, String> {
    let observed = Arc::new(Mutex::new((0_usize, None)));
    let state = Arc::clone(&observed);
    super::super::fixed_census_invocation_observer_v1_tests::with_observer(
        Box::new(move |_, semantic| {
            let mut state = state.lock().unwrap();
            state.0 += 1;
            state.1 = Some(source(semantic, case));
        }),
        || run_production_fixed_checked_output_policy7_extraction_driver_v1(args, artifact),
    )?;
    let (count, source) = Arc::try_unwrap(observed)
        .expect("public invocation observer dropped")
        .into_inner()
        .unwrap();
    if count != 1 {
        return Err(format!("public fixed7 source observations: {count}"));
    }
    let source = source.ok_or("public fixed7 source missing")??;
    let bytes = std::fs::read(artifact).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("public fixed7 emitted no LLVM".into());
    }
    Ok(Outcome::Extracted {
        source,
        llvm_sha256: digest(&bytes),
        llvm_bytes: bytes.len(),
    })
}

#[test]
#[ignore = "strict child; parent supplies exact source/argv and fresh output paths"]
fn policy7_redundant_store_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let request: Request = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    let artifact = PathBuf::from(env::var_os(ARTIFACT).unwrap());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request(&request, &args)?;
        if artifact.exists() {
            return Err("Policy7 artifact is not fresh".into());
        }
        let outcome = match request.mode {
            Mode::Extract => extract(&args, &artifact, request.case)?,
            Mode::Observe => {
                let mut callbacks = SmokeCallbacks {
                    request: request.clone(),
                    artifact,
                    result: None,
                };
                rustc_driver::run_compiler(&args, &mut callbacks);
                callbacks
                    .result
                    .ok_or("actual Policy7 callback did not run")??
            }
        };
        check_request(&request, &args)?;
        Ok(outcome)
    }))
    .unwrap_or_else(|_| Err("rustc or actual Policy7 observation panicked".into()));
    let success = result.is_ok();
    let report = Report { request, result };
    write_new(
        &PathBuf::from(env::var_os(CHILD_RESULT).unwrap()),
        &serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    assert!(
        success,
        "strict actual I-to-J source qualification: {report:?}"
    );
}
fn decode(status: Option<i32>, bytes: Option<&[u8]>, request: &Request) -> Result<Outcome, String> {
    if status != Some(0) {
        return Err(format!("strict Policy7 child exit: {status:?}"));
    }
    let report: Report = serde_json::from_slice(bytes.ok_or("missing fresh Policy7 report")?)
        .map_err(|e| e.to_string())?;
    if &report.request != request {
        return Err("foreign Policy7 request report".into());
    }
    let result = report.result?;
    if !matches!(
        (&result, request.mode),
        (Outcome::Observed(_), Mode::Observe) | (Outcome::Extracted { .. }, Mode::Extract)
    ) {
        return Err("Policy7 outcome mode changed".into());
    }
    if let Outcome::Observed(observed) = &result {
        graph::validate(observed, request.case)?;
    }
    Ok(result)
}
fn child(
    captured: &corpus_cargo::Captured,
    directory: &Path,
    request: Request,
) -> (Outcome, Vec<u8>) {
    let dir = directory.join(format!("{:?}", request.mode));
    std::fs::create_dir(&dir).unwrap();
    let args = dir.join("args.json");
    let report = dir.join("report.json");
    let artifact = dir.join("actual-j.ll");
    write_new(&args, &serde_json::to_vec(&captured.args).unwrap()).unwrap();
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .env_clear()
        .envs(captured.environment.iter().cloned())
        .current_dir(&captured.cwd)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove(CHILD_PROOF_PROBE)
        .env(CHILD_ARGS, &args)
        .env(CHILD_RESULT, &report)
        .env(ARTIFACT, &artifact)
        .env(REQUEST, serde_json::to_string(&request).unwrap())
        .args(["--exact", CHILD, "--ignored", "--nocapture"]);
    progress::clear_inherited_jobserver(&mut command);
    census::configure(&mut command, None);
    let output = command.output().unwrap();
    let bytes = std::fs::read(&report);
    let outcome =
        decode(output.status.code(), bytes.as_deref().ok(), &request).unwrap_or_else(|e| {
            panic!(
                "{:?}: {e}\n{}",
                request.mode,
                corpus_cargo::diagnostics(&output)
            )
        });
    (outcome, std::fs::read(artifact).unwrap())
}

fn capture(case: Case, directory: &Path, target: &Path) -> corpus_cargo::Captured {
    let hash = |p: &str| crate::encode_hex(&digest(&std::fs::read(workspace().join(p)).unwrap()));
    let manifest = format!("{BASE}/Cargo.toml");
    let fixture = corpus::Fixture {
        fixture_id: case.label(),
        target: case.target().name().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/{}", case.fixture()),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.feature().into()],
            kernel_symbols: vec![case.root().into()],
        },
    };
    let mut captured = corpus_cargo::capture(&workspace(), &fixture, directory, target).unwrap();
    require_canonical_overflow_checks_v1(&captured.args).unwrap();
    captured
        .args
        .extend(["-Zinline-mir=no".into(), "-Zmir-opt-level=0".into()]);
    captured
}

fn qualify(captured: &corpus_cargo::Captured, directory: &Path, case: Case) -> graph::Observed {
    let request = Request {
        case,
        mode: Mode::Observe,
        args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
        source: stamps(case),
    };
    let (observed, actual) = child(captured, directory, request.clone());
    let Outcome::Observed(observed) = observed else {
        unreachable!()
    };
    graph::validate(&observed, case).unwrap();
    let (extracted, independent) = child(
        captured,
        directory,
        Request {
            mode: Mode::Extract,
            ..request.clone()
        },
    );
    let Outcome::Extracted {
        source,
        llvm_sha256,
        llvm_bytes,
    } = extracted
    else {
        unreachable!()
    };
    assert_eq!(source, observed.source);
    assert_eq!(
        actual, independent,
        "independent public fixed7 bytes differ from observed actual J"
    );
    assert_eq!(
        (llvm_sha256, llvm_bytes),
        (observed.llvm_sha256, observed.llvm_bytes)
    );
    assert_eq!(digest(&actual), observed.llvm_sha256);
    assert_eq!(stamps(case), request.source);
    *observed
}

#[test]
#[ignore = "strict ordinary Rust gfx942 opt0 private-Store mutation; exactly two compiler children"]
fn ordinary_rust_redundant_private_store_reaches_actual_j_native_and_sim() {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-policy7-store-smoke");
    let captured = capture(
        Case::Smoke {},
        scratch.path(),
        &scratch.path().join("target"),
    );
    let observed = qualify(&captured, scratch.path(), Case::Smoke {});
    eprintln!(
        "POLICY7 SOURCE: 1 direct u32 gfx942 opt0 root; {} actual I-to-J deletions; {} LLVM bytes; {} SIM scenarios; 2 independent compiler children",
        observed.deletions,
        observed.llvm_bytes,
        observed.sim.scenarios.len()
    );
}

#[test]
fn policy7_smoke_protocol_rejects_failure_missing_foreign_and_wrong_mode() {
    let request = Request {
        case: Case::Smoke {},
        mode: Mode::Extract,
        args_sha256: [1; 32],
        source: Vec::new(),
    };
    let bytes = serde_json::to_vec(&Report {
        request: request.clone(),
        result: Ok(Outcome::Extracted {
            source: Source {
                semantic: [2; 32],
                roots: Vec::new(),
            },
            llvm_sha256: [3; 32],
            llvm_bytes: 1,
        }),
    })
    .unwrap();
    assert!(decode(Some(0), Some(&bytes), &request).is_ok());
    for status in [None, Some(1), Some(101)] {
        assert!(decode(status, Some(&bytes), &request).is_err());
    }
    assert!(decode(Some(0), None, &request).is_err());
    assert!(decode(Some(0), Some(b"{}"), &request).is_err());
    let mut foreign = request.clone();
    foreign.args_sha256[0] ^= 1;
    assert!(decode(Some(0), Some(&bytes), &foreign).is_err());
    let wrong_mode = Request {
        mode: Mode::Observe,
        ..request.clone()
    };
    let wrong = serde_json::to_vec(&Report {
        request: wrong_mode.clone(),
        result: Ok(Outcome::Extracted {
            source: Source {
                semantic: [2; 32],
                roots: Vec::new(),
            },
            llvm_sha256: [3; 32],
            llvm_bytes: 1,
        }),
    })
    .unwrap();
    assert!(decode(Some(0), Some(&wrong), &wrong_mode).is_err());
    let refusal = serde_json::to_vec(&Report {
        request: request.clone(),
        result: Err("no actual I duplicate pair".into()),
    })
    .unwrap();
    assert!(decode(Some(0), Some(&refusal), &request).is_err());
}

#[test]
fn policy7_smoke_fixture_wiring_has_module_feature_and_default_exclusion() {
    // Structural wiring only. The ignored source parent executes actual Rust cfg.
    let manifest = include_str!("../tests/fixtures/production-extraction-device/Cargo.toml");
    let lib = include_str!("../tests/fixtures/production-extraction-device/src/lib.rs");
    let check = |lib: &str, manifest: &str| {
        let feature = "feature = \"redundant-store-policy7\"";
        manifest.matches("redundant-store-policy7 = []").count() == 1
            && lib.contains(
                "#[cfg(feature = \"redundant-store-policy7\")]\nmod redundant_store_policy7;",
            )
            && lib.matches(feature).count() == 2
            && lib.split_once("#[cfg(not(any(").is_some_and(|(_, rest)| {
                rest.split_once(")))]")
                    .is_some_and(|(list, _)| list.matches(feature).count() == 1)
            })
    };
    assert!(check(lib, manifest));
    assert!(!check(
        &lib.replace("mod redundant_store_policy7;", ""),
        manifest
    ));
    assert!(!check(
        &lib.replace("    feature = \"redundant-store-policy7\",\n", ""),
        manifest
    ));
    assert!(!check(
        lib,
        &manifest.replace("redundant-store-policy7 = []", "")
    ));
}
