//! Two-profile UnitLocal source parent: two genuine fixed8 invocations per case.
use super::*;
use std::sync::{Arc, Mutex};

const REQUEST: &str = "FE2O3_TEST_UNITLOCAL_POLICY8_REQUEST_V1";
const ARTIFACT: &str = "FE2O3_TEST_UNITLOCAL_POLICY8_ARTIFACT_V1";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::dominance::commutative::fixed_policy8::unit_local::protocol::unitlocal_policy8_source_child";
const LEAF: &str = "commutative_unitlocal_policy8.rs";
const FEATURE: &str = "commutative-unitlocal-policy8";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Mode {
    Observe,
    Extract,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    target: Target,
    mode: Mode,
    args_sha256: [u8; 32],
    source: Vec<FileStamp>,
}
impl Request {
    fn case(&self) -> Case {
        Case {
            integer: Integer::U32,
            target: self.target,
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Response {
    request: Request,
    result: Result<Outcome, String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
enum Outcome {
    Observed(Box<UnitObservation>),
    Extracted {
        source: Source8,
        policy: u16,
        llvm_sha256: [u8; 32],
        llvm_bytes: usize,
    },
}
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn name(target: Target) -> String {
    format!("unitlocal-policy8-u32-{}-opt0", target.cpu())
}
fn stamps(workspace: &Path) -> Vec<FileStamp> {
    [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/{LEAF}"),
    ]
    .into_iter()
    .map(|relative: String| {
        let path = workspace.join(relative).canonicalize().unwrap();
        FileStamp {
            sha256: digest(&std::fs::read(&path).unwrap()),
            path,
        }
    })
    .collect()
}
fn check_request(request: &Request, args: &[String]) -> Result<(), String> {
    if request.source != stamps(&workspace())
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
        return Err("UnitLocal exact current source/argv/retention flags".into());
    }
    require_canonical_overflow_checks_v1(args)
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
fn observe(args: &[String], case: Case, artifact: &Path) -> Result<Outcome, String> {
    let state = Arc::new(Mutex::new((0_usize, None)));
    let output = Arc::clone(&state);
    live::with_observer(
        Box::new(move |view, budget| {
            let mut state = output.lock().unwrap();
            state.0 += 1;
            if state.0 != 1 {
                return Err("duplicate actual Erased8 observation".into());
            }
            state.1 = Some(observe_erased_view(view, case, budget)?);
            Ok(())
        }),
        || crate::run_production_fixed_checked_output_policy8_extraction_driver_v1(args, artifact),
    )?;
    let (calls, observed) = Arc::try_unwrap(state)
        .expect("actual observer dropped")
        .into_inner()
        .unwrap();
    if calls != 1 {
        return Err("actual Erased8 callback must run exactly once".into());
    }
    let report = observed.ok_or("actual Erased8 owner was not observed")?;
    let bytes = std::fs::read(artifact).map_err(|e| e.to_string())?;
    if (digest(&bytes), bytes.len())
        != (
            report.common.native_llvm_sha256,
            report.common.native_llvm_bytes,
        )
    {
        return Err("complete actual K artifact differs from borrowed Erased8 report".into());
    }
    validate_unit(&report, case)?;
    Ok(Outcome::Observed(Box::new(report)))
}
fn extract(args: &[String], artifact: &Path) -> Result<Outcome, String> {
    match super::super::protocol8::extract8(args, artifact)? {
        super::super::protocol8::Outcome8::Extracted {
            source,
            policy,
            llvm_sha256,
            llvm_bytes,
        } => Ok(Outcome::Extracted {
            source,
            policy,
            llvm_sha256,
            llvm_bytes,
        }),
        _ => Err("independent literal fixed8 invocation returned an observation".into()),
    }
}

#[test]
#[ignore = "strict child requiring exact parent request and fresh response/artifact"]
fn unitlocal_policy8_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let request: Request = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    let artifact = PathBuf::from(env::var_os(ARTIFACT).unwrap());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request(&request, &args)?;
        if artifact.exists() {
            return Err("UnitLocal artifact is not fresh".into());
        }
        let outcome = match request.mode {
            Mode::Observe => observe(&args, request.case(), &artifact)?,
            Mode::Extract => extract(&args, &artifact)?,
        };
        check_request(&request, &args)?;
        Ok(outcome)
    }))
    .unwrap_or_else(|_| Err("rustc or UnitLocal source/N/E/J/K observation panicked".into()));
    let success = result.is_ok();
    let response = Response { request, result };
    write_new(
        &PathBuf::from(env::var_os(CHILD_RESULT).unwrap()),
        &serde_json::to_vec(&response).unwrap(),
    )
    .unwrap();
    assert!(success, "strict actual UnitLocal K qualifier: {response:?}");
}
fn decode(status: Option<i32>, bytes: Option<&[u8]>, request: &Request) -> Result<Outcome, String> {
    if status != Some(0) {
        return Err(format!("strict UnitLocal child exit: {status:?}"));
    }
    let bytes = bytes.ok_or("missing fresh UnitLocal report")?;
    let response: Response = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    // Some reused diagnostic structures predate deny_unknown_fields. Require a
    // lossless typed decode so extra fields at any depth cannot disappear.
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if serde_json::to_value(&response).map_err(|e| e.to_string())? != raw
        || &response.request != request
    {
        return Err("foreign request or noncanonical nested UnitLocal report schema".into());
    }
    let outcome = response.result?;
    match (&outcome, request.mode) {
        (Outcome::Observed(report), Mode::Observe) => validate_unit(report, request.case())?,
        (
            Outcome::Extracted {
                source,
                policy: 8,
                llvm_sha256,
                llvm_bytes,
            },
            Mode::Extract,
        ) if source.semantic != [0; 32] && *llvm_sha256 != [0; 32] && *llvm_bytes > 0 => {
            graph::roster(source.roots.iter().map(|r| r.name.as_str()))?;
        }
        _ => return Err("UnitLocal exact mode/native policy/output".into()),
    }
    Ok(outcome)
}
fn child(
    captured: &corpus_cargo::Captured,
    directory: &Path,
    request: Request,
) -> (Outcome, Vec<u8>) {
    let directory = directory.join(format!("{:?}", request.mode));
    std::fs::create_dir(&directory).unwrap();
    let args = directory.join("args.json");
    let report = directory.join("report.json");
    let artifact = directory.join("actual-unitlocal-k.ll");
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
                "{} {:?}: {e}\n{}",
                name(request.target),
                request.mode,
                corpus_cargo::diagnostics(&output)
            )
        });
    (outcome, std::fs::read(artifact).unwrap())
}

#[test]
#[ignore = "strict ordinary UnitLocal u32 K mutation on both profiles; four compiler children"]
fn ordinary_rust_unitlocal_actual_k_native_both_profiles() {
    let workspace = workspace();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-unitlocal-k-native");
    let mut completed = 0;
    let mut scenarios = 0;
    let mut erased_calls = 0;
    let mut substitutions = 0;
    for target in [Target::Gfx942, Target::Gfx950] {
        let case = Case {
            integer: Integer::U32,
            target,
        };
        let directory = scratch.path().join(name(target));
        std::fs::create_dir(&directory).unwrap();
        let source = stamps(&workspace);
        let mut fixture = fixture_for(&workspace, case, FEATURE, LEAF, name(target));
        fixture.compiler_input.features = vec![FEATURE.into()];
        let mut captured = corpus_cargo::capture(
            &workspace,
            &fixture,
            &directory,
            &scratch.path().join(target.cpu()),
        )
        .unwrap();
        assert_eq!(stamps(&workspace), source);
        require_canonical_overflow_checks_v1(&captured.args).unwrap();
        captured
            .args
            .extend(["-Zmir-opt-level=0".into(), "-Zinline-mir=no".into()]);
        let request = Request {
            target,
            mode: Mode::Observe,
            args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
            source: source.clone(),
        };
        let (observed, baseline) = child(&captured, &directory, request.clone());
        let Outcome::Observed(observed) = observed else {
            panic!("wrong UnitLocal observation mode");
        };
        validate_unit(&observed, case).unwrap();
        let (extracted, independent) = child(
            &captured,
            &directory,
            Request {
                mode: Mode::Extract,
                ..request
            },
        );
        let Outcome::Extracted {
            source: actual_source,
            policy,
            llvm_sha256,
            llvm_bytes,
        } = extracted
        else {
            panic!("wrong public fixed8 extraction mode");
        };
        assert_eq!(actual_source, observed.common.source);
        assert_eq!(policy, 8);
        assert_eq!(
            baseline, independent,
            "complete independent public K LLVM plus descriptor bytes"
        );
        assert_eq!(
            (digest(&baseline), baseline.len()),
            (
                observed.common.native_llvm_sha256,
                observed.common.native_llvm_bytes
            )
        );
        assert_eq!(
            (llvm_sha256, llvm_bytes),
            (
                observed.common.native_llvm_sha256,
                observed.common.native_llvm_bytes
            )
        );
        assert_eq!(stamps(&workspace), source);
        completed += 1;
        scenarios += observed.common.compared_scenarios;
        erased_calls += observed.helper_erasure.erased_call_count;
        substitutions += observed.common.proved_pairs;
        eprintln!(
            "ACTUAL UNITLOCAL K {}: 3 original-N helper calls erased in N/E; 3 distinct J/K commutative substitutions; 360 paired N/K SIM scenarios",
            name(target)
        );
    }
    assert_eq!(
        (
            completed,
            completed * ROOTS.len(),
            completed * 2,
            scenarios,
            erased_calls,
            substitutions
        ),
        (2, 6, 4, 720, 6, 6)
    );
}

fn request_fixture() -> Request {
    Request {
        target: Target::Gfx942,
        mode: Mode::Extract,
        args_sha256: [1; 32],
        source: vec![],
    }
}
fn response_fixture(request: Request) -> Vec<u8> {
    serde_json::to_vec(&Response {
        request,
        result: Ok(Outcome::Extracted {
            source: Source8 {
                semantic: [4; 32],
                roots: ROOTS
                    .map(|name| census::SourceRoot {
                        name: name.into(),
                        function: [2; 32],
                        body: [3; 32],
                    })
                    .to_vec(),
            },
            policy: 8,
            llvm_sha256: [5; 32],
            llvm_bytes: 100,
        }),
    })
    .unwrap()
}
#[test]
fn unitlocal_protocol_rejects_refusals_status_modes_and_foreign_requests() {
    let request = request_fixture();
    let bytes = response_fixture(request.clone());
    assert!(decode(Some(0), Some(&bytes), &request).is_ok());
    for status in [None, Some(1), Some(101)] {
        assert!(decode(status, Some(&bytes), &request).is_err());
    }
    assert!(decode(Some(0), None, &request).is_err());
    assert!(decode(Some(0), Some(b"{}"), &request).is_err());
    for mutate in [
        |r: &mut Request| r.target = Target::Gfx950,
        |r: &mut Request| r.mode = Mode::Observe,
        |r: &mut Request| r.args_sha256[0] ^= 1,
        |r: &mut Request| {
            r.source.push(FileStamp {
                path: "foreign".into(),
                sha256: [6; 32],
            })
        },
    ] {
        let mut changed = request.clone();
        mutate(&mut changed);
        assert!(decode(Some(0), Some(&response_fixture(changed)), &request).is_err());
    }
    let changed = Request {
        mode: Mode::Observe,
        ..request.clone()
    };
    assert!(decode(Some(0), Some(&response_fixture(changed.clone())), &changed).is_err());
    let refusal = serde_json::to_vec(&Response {
        request: request.clone(),
        result: Err("source admission refusal".into()),
    })
    .unwrap();
    assert!(decode(Some(0), Some(&refusal), &request).is_err());
}
#[test]
fn unitlocal_protocol_rejects_nested_unknown_fields_tags_and_old_j_policy() {
    let request = request_fixture();
    let good: serde_json::Value =
        serde_json::from_slice(&response_fixture(request.clone())).unwrap();
    for pointer in [
        "/result/Ok/Extracted",
        "/result/Ok/Extracted/source",
        "/result/Ok/Extracted/source/roots/0",
        "/request",
    ] {
        let mut changed = good.clone();
        changed
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), true.into());
        assert!(
            decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                &request
            )
            .is_err()
        );
    }
    let mut changed = good.clone();
    changed["result"]["Ok"]["Extracted"]["policy"] = 7.into();
    assert!(
        decode(
            Some(0),
            Some(&serde_json::to_vec(&changed).unwrap()),
            &request
        )
        .is_err()
    );
    let mut changed = good.clone();
    let tagged = changed["result"]["Ok"].as_object_mut().unwrap();
    let fields = tagged.remove("Extracted").unwrap();
    tagged.insert("Direct".into(), fields);
    assert!(
        decode(
            Some(0),
            Some(&serde_json::to_vec(&changed).unwrap()),
            &request
        )
        .is_err()
    );
    let mut changed = good;
    changed["result"]["Ok"]["Observed"] = serde_json::Value::Null;
    assert!(
        decode(
            Some(0),
            Some(&serde_json::to_vec(&changed).unwrap()),
            &request
        )
        .is_err()
    );
    let mut helper = serde_json::to_value(erasure_fixture()).unwrap();
    helper["source_calls"][0]["unexpected"] = true.into();
    assert!(serde_json::from_value::<ErasedHelperCalls>(helper).is_err());
}
#[test]
fn unitlocal_request_binds_exact_source_and_retention_flags() {
    let args = [
        "rustc",
        "-Coverflow-checks=on",
        "-Zmir-opt-level=0",
        "-Zinline-mir=no",
    ]
    .map(str::to_owned)
    .to_vec();
    let request = Request {
        source: stamps(&workspace()),
        args_sha256: digest(&serde_json::to_vec(&args).unwrap()),
        ..request_fixture()
    };
    check_request(&request, &args).unwrap();
    for mutate in [
        |r: &mut Request| r.source[3].sha256[0] ^= 1,
        |r: &mut Request| r.source.swap(0, 1),
        |r: &mut Request| r.source.push(r.source[0].clone()),
        |r: &mut Request| r.args_sha256[0] ^= 1,
    ] {
        let mut changed = request.clone();
        mutate(&mut changed);
        assert!(check_request(&changed, &args).is_err());
    }
    for extra in [
        "-Coverflow-checks=on",
        "-Coverflow-checks=off",
        "-Zmir-opt-level=0",
        "-Zinline-mir=no",
    ] {
        let mut changed = args.clone();
        changed.push(extra.into());
        let request = Request {
            args_sha256: digest(&serde_json::to_vec(&changed).unwrap()),
            ..request.clone()
        };
        assert!(check_request(&request, &changed).is_err());
    }
    for omit in [1, 2, 3] {
        let mut changed = args.clone();
        changed.remove(omit);
        let request = Request {
            args_sha256: digest(&serde_json::to_vec(&changed).unwrap()),
            ..request.clone()
        };
        assert!(check_request(&request, &changed).is_err());
    }
}
