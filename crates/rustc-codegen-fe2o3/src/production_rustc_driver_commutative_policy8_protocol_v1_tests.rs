//! Separate strict subprocess protocol; old fixed6/fixed7 protocols are unchanged.
use super::*;
use std::sync::{Arc, Mutex};

const REQUEST8: &str = "FE2O3_TEST_COMMUTATIVE_POLICY8_REQUEST_V1";
const ARTIFACT8: &str = "FE2O3_TEST_COMMUTATIVE_POLICY8_ARTIFACT_V1";
const CHILD8: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::dominance::commutative::fixed_policy8::protocol8::commutative_policy8_source_child";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Mode8 {
    Observe,
    Extract,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request8 {
    case: Case,
    mode: Mode8,
    args_sha256: [u8; 32],
    source: Vec<FileStamp>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Response8 {
    request: Request8,
    result: Result<Outcome8, String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) enum Outcome8 {
    Observed(Box<Observation8>),
    Extracted {
        source: Source8,
        policy: u16,
        llvm_sha256: [u8; 32],
        llvm_bytes: usize,
    },
}

fn workspace8() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn name8(case: Case) -> String {
    format!(
        "post-policy8-{}-{}-opt0",
        case.integer.name(),
        case.target.cpu()
    )
}
fn check_request8(request: &Request8, args: &[String]) -> Result<(), String> {
    if request.source != source_stamps(&workspace8(), FixtureCase::CommutativeCse(request.case))
        || request.args_sha256 != digest(&serde_json::to_vec(args).map_err(|e| e.to_string())?)
        || args
            .iter()
            .filter(|a| a.starts_with("-Zmir-opt-level"))
            .map(String::as_str)
            .collect::<Vec<_>>()
            != ["-Zmir-opt-level=0"]
        || args
            .iter()
            .filter(|a| a.starts_with("-Zinline-mir"))
            .map(String::as_str)
            .collect::<Vec<_>>()
            != ["-Zinline-mir=no"]
    {
        return Err(
            "tail qualifier requires exact current source, argv, and opt0 retention".into(),
        );
    }
    require_canonical_overflow_checks_v1(args)
}
fn write_new8(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| e.to_string())
}

fn observe8(args: &[String], case: Case, artifact: &Path) -> Result<Outcome8, String> {
    let state = Arc::new(Mutex::new((0_usize, None)));
    let output = Arc::clone(&state);
    live::with_observer(
        Box::new(move |view, budget| {
            let mut state = output.lock().unwrap();
            state.0 += 1;
            if state.0 != 1 {
                return Err("duplicate actual K stage observation".into());
            }
            state.1 = Some(observe_view(view, case, budget)?);
            Ok(())
        }),
        || crate::run_production_fixed_checked_output_policy8_extraction_driver_v1(args, artifact),
    )?;
    let (calls, observed) = Arc::try_unwrap(state)
        .expect("actual K observer dropped")
        .into_inner()
        .unwrap();
    if calls != 1 {
        return Err("actual K callback did not run exactly once".into());
    }
    let report = observed.ok_or("actual source-owned K was not observed")?;
    let bytes = std::fs::read(artifact).map_err(|e| format!("missing actual K artifact: {e}"))?;
    if (digest(&bytes), bytes.len()) != (report.native_llvm_sha256, report.native_llvm_bytes) {
        return Err("actual K native bytes differ from borrowed stage report".into());
    }
    validate8(&report, case)?;
    Ok(Outcome8::Observed(Box::new(report)))
}

pub(super) fn extract8(args: &[String], artifact: &Path) -> Result<Outcome8, String> {
    let state = Arc::new(Mutex::new((0_usize, None)));
    let output = Arc::clone(&state);
    crate::production_rustc_driver_v1::fixed_census_invocation_observer_v1_tests::with_observer(
        Box::new(move |_, semantic| {
            let mut state = output.lock().unwrap();
            state.0 += 1;
            state.1 = Some(source8(semantic));
        }),
        || crate::run_production_fixed_checked_output_policy8_extraction_driver_v1(args, artifact),
    )?;
    let (count, source) = Arc::try_unwrap(state)
        .expect("public observer dropped")
        .into_inner()
        .unwrap();
    if count != 1 {
        return Err("public fixed8 actual source callback count".into());
    }
    let source = source.ok_or("public fixed8 source missing")??;
    let bytes = std::fs::read(artifact).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("public fixed8 baseline is empty".into());
    }
    Ok(Outcome8::Extracted {
        source,
        policy: 8,
        llvm_sha256: digest(&bytes),
        llvm_bytes: bytes.len(),
    })
}

#[test]
#[ignore = "strict child requiring an exact parent request and fresh report/baseline paths"]
fn commutative_policy8_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let request: Request8 = serde_json::from_str(&env::var(REQUEST8).unwrap()).unwrap();
    let artifact = PathBuf::from(env::var_os(ARTIFACT8).unwrap());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request8(&request, &args)?;
        if artifact.exists() {
            return Err("tail baseline output is not fresh".into());
        }
        let outcome = match request.mode {
            Mode8::Extract => extract8(&args, &artifact)?,
            Mode8::Observe => observe8(&args, request.case, &artifact)?,
        };
        check_request8(&request, &args)?;
        Ok(outcome)
    }))
    .unwrap_or_else(|_| Err("rustc or actual source/P7/J/K observation panicked".into()));
    let success = result.is_ok();
    let response = Response8 { request, result };
    write_new8(
        &PathBuf::from(env::var_os(CHILD_RESULT).unwrap()),
        &serde_json::to_vec(&response).unwrap(),
    )
    .unwrap();
    assert!(
        success,
        "strict actual source-owned J/K qualifier: {response:?}"
    );
}

fn decode8(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    request: &Request8,
) -> Result<Outcome8, String> {
    if status != Some(0) {
        return Err(format!("strict tail child exit: {status:?}"));
    }
    let response: Response8 = serde_json::from_slice(bytes.ok_or("missing fresh tail report")?)
        .map_err(|e| e.to_string())?;
    if &response.request != request {
        return Err("foreign tail request/argv/source report".into());
    }
    let outcome = response.result?;
    match (&outcome, request.mode) {
        (Outcome8::Observed(report), Mode8::Observe) => validate8(report, request.case)?,
        (
            Outcome8::Extracted {
                source,
                policy: 8,
                llvm_sha256,
                llvm_bytes,
            },
            Mode8::Extract,
        ) if *llvm_sha256 != [0; 32] && *llvm_bytes > 0 && source.semantic != [0; 32] => {
            graph::roster(source.roots.iter().map(|r| r.name.as_str()))?;
        }
        _ => return Err("fixed8 report mode or actual native policy changed".into()),
    }
    Ok(outcome)
}
fn child8(
    captured: &corpus_cargo::Captured,
    directory: &Path,
    request: Request8,
) -> (Outcome8, Vec<u8>) {
    let directory = directory.join(format!("{:?}", request.mode));
    std::fs::create_dir(&directory).unwrap();
    let args = directory.join("args.json");
    let report = directory.join("report.json");
    let artifact = directory.join("actual-p8-k.ll");
    write_new8(&args, &serde_json::to_vec(&captured.args).unwrap()).unwrap();
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
        .env(ARTIFACT8, &artifact)
        .env(REQUEST8, serde_json::to_string(&request).unwrap())
        .args(["--exact", CHILD8, "--ignored", "--nocapture"]);
    progress::clear_inherited_jobserver(&mut command);
    census::configure(&mut command, None);
    let output = command.output().unwrap();
    let bytes = std::fs::read(&report);
    let result =
        decode8(output.status.code(), bytes.as_deref().ok(), &request).unwrap_or_else(|e| {
            panic!(
                "{} {:?}: {e}\n{}",
                name8(request.case),
                request.mode,
                corpus_cargo::diagnostics(&output)
            )
        });
    (result, std::fs::read(artifact).unwrap())
}

#[test]
#[ignore = "strict actual K native mutation on all eight widths/both profiles; 32 compiler children"]
fn ordinary_rust_actual_k_native_all_widths_and_profiles() {
    let workspace = workspace8();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-actual-k-native");
    let mut completed = 0;
    let mut compared = 0;
    for case in cases() {
        let directory = scratch.path().join(name8(case));
        std::fs::create_dir(&directory).unwrap();
        let source = source_stamps(&workspace, FixtureCase::CommutativeCse(case));
        let mut captured = corpus_cargo::capture(
            &workspace,
            &fixture_for(
                &workspace,
                case,
                "commutative-cse",
                "commutative_cse.rs",
                name8(case),
            ),
            &directory,
            &scratch.path().join(case.target.cpu()),
        )
        .unwrap();
        require_canonical_overflow_checks_v1(&captured.args).unwrap();
        captured
            .args
            .extend(["-Zmir-opt-level=0".into(), "-Zinline-mir=no".into()]);
        let request = Request8 {
            case,
            mode: Mode8::Observe,
            args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
            source: source.clone(),
        };
        let (observed, baseline) = child8(&captured, &directory, request.clone());
        let Outcome8::Observed(observed) = observed else {
            panic!("wrong tail observation mode");
        };
        validate8(&observed, case).unwrap();
        let (extracted, independent) = child8(
            &captured,
            &directory,
            Request8 {
                mode: Mode8::Extract,
                ..request
            },
        );
        let Outcome8::Extracted {
            source: actual_source,
            policy,
            llvm_sha256,
            llvm_bytes,
        } = extracted
        else {
            panic!("wrong public fixed8 baseline mode");
        };
        assert_eq!(actual_source, observed.source);
        assert_eq!(policy, 8);
        assert_eq!(
            baseline, independent,
            "independent public fixed8 actual K bytes"
        );
        assert_eq!(
            (digest(&baseline), baseline.len()),
            (observed.native_llvm_sha256, observed.native_llvm_bytes)
        );
        assert_eq!(
            (llvm_sha256, llvm_bytes),
            (observed.native_llvm_sha256, observed.native_llvm_bytes)
        );
        assert_eq!(
            source_stamps(&workspace, FixtureCase::CommutativeCse(case)),
            source
        );
        completed += 1;
        compared += observed.compared_scenarios;
        eprintln!(
            "ACTUAL J/K {}: 3 checked commutative substitutions; original N/final K {} paired SIM scenarios; native output is policy8 K",
            name8(case),
            observed.compared_scenarios
        );
    }
    assert_eq!(
        (completed, completed * ROOTS.len(), completed * 2, compared),
        (16, 48, 32, 5760)
    );
}

#[test]
fn actual_k_protocol_never_accepts_refusal_missing_foreign_or_wrong_mode() {
    let request = Request8 {
        case: cases()[0],
        mode: Mode8::Extract,
        args_sha256: [1; 32],
        source: vec![],
    };
    let roots = ROOTS
        .map(|name| census::SourceRoot {
            name: name.into(),
            function: [2; 32],
            body: [3; 32],
        })
        .to_vec();
    let encode = |request: Request8| {
        serde_json::to_vec(&Response8 {
            request,
            result: Ok(Outcome8::Extracted {
                source: Source8 {
                    semantic: [4; 32],
                    roots: roots.clone(),
                },
                policy: 8,
                llvm_sha256: [5; 32],
                llvm_bytes: 100,
            }),
        })
        .unwrap()
    };
    let bytes = encode(request.clone());
    assert!(decode8(Some(0), Some(&bytes), &request).is_ok());
    let canonical: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut extra_field = canonical.clone();
    extra_field["result"]["Ok"]["Extracted"]["unexpected"] = true.into();
    assert!(
        decode8(
            Some(0),
            Some(&serde_json::to_vec(&extra_field).unwrap()),
            &request
        )
        .is_err()
    );
    let mut unknown_tag = canonical.clone();
    let outcome = unknown_tag["result"]["Ok"].as_object_mut().unwrap();
    let fields = outcome.remove("Extracted").unwrap();
    outcome.insert("UnexpectedOutcome".into(), fields);
    assert!(
        decode8(
            Some(0),
            Some(&serde_json::to_vec(&unknown_tag).unwrap()),
            &request
        )
        .is_err()
    );
    let mut extra_tag = canonical;
    extra_tag["result"]["Ok"]["UnexpectedOutcome"] = serde_json::Value::Null;
    assert!(
        decode8(
            Some(0),
            Some(&serde_json::to_vec(&extra_tag).unwrap()),
            &request
        )
        .is_err()
    );
    for status in [None, Some(1), Some(101)] {
        assert!(decode8(status, Some(&bytes), &request).is_err());
    }
    assert!(decode8(Some(0), None, &request).is_err());
    assert!(decode8(Some(0), Some(b"{}"), &request).is_err());
    let mut changed = request.clone();
    changed.args_sha256[0] ^= 1;
    assert!(decode8(Some(0), Some(&encode(changed)), &request).is_err());
    let mut changed = request.clone();
    changed.case = cases()[1];
    assert!(decode8(Some(0), Some(&encode(changed)), &request).is_err());
    let mut changed = request.clone();
    changed.mode = Mode8::Observe;
    assert!(decode8(Some(0), Some(&encode(changed.clone())), &changed).is_err());
    let refusal = serde_json::to_vec(&Response8 {
        request: request.clone(),
        result: Err("source refusal".into()),
    })
    .unwrap();
    assert!(decode8(Some(0), Some(&refusal), &request).is_err());
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["result"]["Ok"]["Extracted"]["policy"] = 7.into();
    assert!(
        decode8(
            Some(0),
            Some(&serde_json::to_vec(&value).unwrap()),
            &request
        )
        .is_err()
    );
}

#[test]
fn actual_k_request_requires_exact_source_argv_and_canonical_overflow_opt0() {
    let args = [
        "rustc",
        "-Coverflow-checks=on",
        "-Zmir-opt-level=0",
        "-Zinline-mir=no",
    ]
    .map(str::to_owned)
    .to_vec();
    let case = cases()[0];
    let request = Request8 {
        case,
        mode: Mode8::Observe,
        args_sha256: digest(&serde_json::to_vec(&args).unwrap()),
        source: source_stamps(&workspace8(), FixtureCase::CommutativeCse(case)),
    };
    check_request8(&request, &args).unwrap();
    let mut changed = request.clone();
    changed.source[0].sha256[0] ^= 1;
    assert!(check_request8(&changed, &args).is_err());
    let mut changed = request.clone();
    changed.source.swap(0, 1);
    assert!(check_request8(&changed, &args).is_err());
    let mut changed = request.clone();
    changed.source.push(changed.source[0].clone());
    assert!(check_request8(&changed, &args).is_err());
    let mut changed = request.clone();
    changed.args_sha256[0] ^= 1;
    assert!(check_request8(&changed, &args).is_err());
    for extra in [
        "-Coverflow-checks=on",
        "-Coverflow-checks=off",
        "-Zmir-opt-level=0",
        "-Zinline-mir=no",
    ] {
        let mut altered = args.clone();
        altered.push(extra.into());
        let changed = Request8 {
            args_sha256: digest(&serde_json::to_vec(&altered).unwrap()),
            ..request.clone()
        };
        assert!(check_request8(&changed, &altered).is_err());
    }
    for omit in [1, 2, 3] {
        let mut altered = args.clone();
        altered.remove(omit);
        let changed = Request8 {
            args_sha256: digest(&serde_json::to_vec(&altered).unwrap()),
            ..request.clone()
        };
        assert!(check_request8(&changed, &altered).is_err());
    }
}
