//! Strict consuming ordinary-source neutral-history qualification, before targets.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionCanonicalScalarFixedPointOwnerV1 as NeutralOwner,
    ProductionCanonicalScalarSourceErrorV1,
};
use std::io::{Read, Write};

const REQUEST_ENV: &str = "FE2O3_TEST_CANONICAL_SCALAR_REQUEST_V1";
const ARGS_ENV: &str = "FE2O3_TEST_CANONICAL_SCALAR_ARGS_V1";
const RESULT_ENV: &str = "FE2O3_TEST_CANONICAL_SCALAR_RESULT_V1";
const CHILD_NAME: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::canonical_scalar_source::canonical_scalar_source_child";
const MUTATING_CFG: &str = "fe2o3_canonical_scalar_mutating";
const NOOP_CFG: &str = "fe2o3_canonical_scalar_noop";
const INPUT_CAP: usize = 1024 * 1024;
const REPORT_CAP: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum NeutralCase {
    MutatingOpt0,
    NormalNoop,
}
impl NeutralCase {
    fn root(self) -> &'static str {
        match self {
            Self::MutatingOpt0 => "scalar_neutral_mutating",
            Self::NormalNoop => "scalar_neutral_noop",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NeutralRequest {
    schema: u16,
    run_id: String,
    case: NeutralCase,
    target: Target,
    captured: [u8; 32],
    executed: [u8; 32],
    cwd: PathBuf,
    source: [FileStamp; 4],
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    captured: Vec<String>,
    executed: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    digest: [u8; 32],
    bytes: usize,
}
fn subject(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12) -> Subject {
    Subject {
        digest: *owner.canonical().identity().digest(),
        bytes: owner.canonical().canonical_bytes().len(),
    }
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Round {
    ordinal: u16,
    input: Subject,
    integer: Subject,
    output: Subject,
    changed: bool,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Root {
    name: String,
    function: [u8; 32],
    body: [u8; 32],
    entry: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Checks {
    functions: usize,
    stages: usize,
    pending: usize,
    spans: usize,
    zero_spans: usize,
    definitions: usize,
    substitutions: usize,
    elisions: usize,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    actual_target: Target,
    original: Subject,
    output: Subject,
    roots: Vec<Root>,
    rounds: Vec<Round>,
    before_operations: usize,
    after_operations: usize,
    checks: Checks,
    source_floor: usize,
    additional: usize,
    owner_floor: usize,
    final_storage: usize,
    work: usize,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: NeutralRequest,
    callbacks: usize,
    result: Result<Observation, String>,
}

fn require(ok: bool, reason: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(reason.into()) }
}
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}
fn stamps() -> Result<[FileStamp; 4], String> {
    let base = workspace();
    [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/scalar_fixed_point.rs"),
    ]
    .into_iter()
    .map(|path: String| {
        let path = base.join(path).canonicalize().map_err(|e| e.to_string())?;
        let sha256 = digest(&std::fs::read(&path).map_err(|e| e.to_string())?);
        Ok(FileStamp { path, sha256 })
    })
    .collect::<Result<Vec<_>, String>>()?
    .try_into()
    .map_err(|_| "source roster".into())
}
fn args_hash(args: &[String]) -> [u8; 32] {
    digest(&serde_json::to_vec(args).unwrap())
}
fn executed(captured: &[String], case: NeutralCase) -> Result<Vec<String>, String> {
    require_canonical_overflow_checks_v1(captured).map_err(|e| e.to_string())?;
    require(
        !captured.iter().any(|arg| {
            arg.contains("fe2o3_scalar_fixed_point_")
                || arg.contains("fe2o3_canonical_scalar_")
                || arg.contains("mir-opt-level")
                || arg.contains("inline-mir")
        }),
        "preexisting fixture cfg or MIR override",
    )?;
    let mut args = captured.to_vec();
    for cfg in [MUTATING_CFG, NOOP_CFG] {
        args.push(format!("--check-cfg=cfg({cfg})"));
    }
    match case {
        NeutralCase::MutatingOpt0 => {
            args.push(format!("--cfg={MUTATING_CFG}"));
            args.push("-Zmir-opt-level=0".into());
        }
        NeutralCase::NormalNoop => args.push(format!("--cfg={NOOP_CFG}")),
    }
    Ok(args)
}
fn options<'a>(args: &'a [String], prefix: &str) -> Vec<&'a str> {
    args.iter()
        .enumerate()
        .filter_map(|(n, arg)| {
            if arg == prefix {
                args.get(n + 1).map(String::as_str)
            } else {
                arg.strip_prefix(&format!("{prefix}="))
            }
        })
        .collect()
}
fn check_request(request: &NeutralRequest, invocation: &Invocation) -> Result<(), String> {
    require(
        request.schema == 1
            && !request.run_id.is_empty()
            && request.run_id.len() <= 256
            && request.source == stamps()?
            && request.cwd == env::current_dir().map_err(|e| e.to_string())?
            && request
                .source
                .iter()
                .map(|s| &s.path)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == 4
            && request.captured == args_hash(&invocation.captured)
            && request.executed == args_hash(&invocation.executed)
            && invocation.executed == executed(&invocation.captured, request.case)?,
        "exact source/cwd/argv request binding",
    )?;
    require(
        options(&invocation.captured, "-Ctarget-cpu") == [request.target.cpu()]
            && options(&invocation.captured, "--crate-name")
                == ["fe2o3_production_extraction_fixture"]
            && invocation
                .captured
                .iter()
                .filter(|arg| {
                    request.cwd.join(arg).canonicalize().ok()
                        == Some(request.source[2].path.clone())
                })
                .count()
                == 1,
        "captured target/crate/source entry",
    )
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path, cap: usize) -> Result<T, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    require(bytes.len() <= cap, "bounded JSON exceeded")?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn write_json(path: &Path, value: &impl Serialize, cap: usize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    require(bytes.len() <= cap, "bounded JSON exceeded")?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|e| e.to_string())
}
fn operations(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12) -> usize {
    owner
        .module()
        .functions
        .iter()
        .flat_map(|f| f.body.iter())
        .flat_map(|b| &b.blocks)
        .map(|b| b.operations.len())
        .sum()
}
fn validate(request: &NeutralRequest, row: &Observation) -> Result<(), String> {
    require(
        row.actual_target == request.target
            && row.roots.len() == 1
            && row.roots[0].name == request.case.root()
            && row.roots[0].function != [0; 32]
            && row.roots[0].body != [0; 32]
            && !row.roots[0].entry.is_empty()
            && row.original.digest != [0; 32]
            && row.output.digest != [0; 32]
            && row.original.bytes > 0
            && row.output.bytes > 0,
        "actual ordered source and neutral subjects",
    )?;
    require(
        row.checks.functions == 1
            && row.checks.stages == 9
            && row.checks.pending == 19
            && row.checks.spans > 0
            && row.checks.zero_spans > 0
            && row.source_floor > 0
            && row.additional > 0
            && row.source_floor.checked_add(row.additional) == Some(row.owner_floor)
            && row.final_storage == 0
            && row.work > 0,
        "real final fixed-nine/source/floor contract",
    )?;
    require(
        !row.rounds.is_empty()
            && row.rounds.len() <= fe2o3_kernel_opt::SCALAR_FIXED_POINT_MAX_ROUNDS_V1,
        "complete round extent",
    )?;
    let mut input = &row.original;
    for (n, round) in row.rounds.iter().enumerate() {
        require(
            round.ordinal as usize == n
                && &round.input == input
                && round.integer.bytes > 0
                && round.integer.digest != [0; 32]
                && round.output.bytes > 0
                && round.output.digest != [0; 32]
                && round.changed == (round.input != round.output)
                && round.changed == (n + 1 != row.rounds.len()),
            "actual adjacent terminal schedule",
        )?;
        input = &round.output;
    }
    require(
        input == &row.output
            && match request.case {
                NeutralCase::MutatingOpt0 => {
                    row.original != row.output
                        && row.rounds.len() >= 2
                        && row.before_operations > row.after_operations
                        && row.before_operations > 0
                        && row.checks.definitions > 0
                        && row.checks.elisions > 0
                }
                NeutralCase::NormalNoop => {
                    row.original == row.output
                        && row.rounds.len() == 1
                        && row.before_operations == 0
                        && row.after_operations == 0
                }
            },
        "nonvacuous mutation or exact neutral no-op",
    )
}

fn observe(tcx: TyCtxt<'_>, request: &NeutralRequest) -> Result<Observation, String> {
    let cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    require(
        cpu == request.target.cpu(),
        "actual compiler session target",
    )?;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| e.to_string())?;
    transaction.consume_pre_ranked_for_test_v1(|source, descriptors| {
        require(descriptors.len() == 1, "actual descriptor roster")?;
        let semantic = source.semantic_ssa().source_semantic();
        let source_roots = census::roots(semantic)?;
        require(source_roots.len() == source.executable().module().kernels.len(), "complete source and executable root roster")?;
        let roots = source_roots.into_iter().zip(&source.executable().module().kernels)
            .map(|(root, kernel)| Root { name: root.name, function: root.function, body: root.body, entry: kernel.entry.as_str().into() }).collect();
        let original = subject(source.executable());
        let before_operations = operations(source.executable());
        let source_floor = source.unit_local_source_storage_floor_v1().map_err(|e| e.to_string())?;
        let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
        let mut budget = Budget::new(&mut work, crate::production_canonical_phase_policy_v1::STORAGE_LIMIT);
        budget.reserve_storage(source_floor).map_err(|e| e.to_string())?;
        let (owner, receipt) = NeutralOwner::try_prepare_v1(source, &mut budget).map_err(|e| e.to_string())?;
        require(budget.storage() == source_floor, "consuming factory restores source floor")?;
        let additional = receipt.retained_storage();
        budget.reserve_storage(additional).map_err(|e| e.to_string())?;
        let owner_floor = owner.retained_storage_floor_v1();
        require(budget.storage() == owner_floor && subject(owner.original_source().executable()) == original, "owned original and actual history")?;
        let output = subject(owner.output());
        let after_operations = operations(owner.output());
        // Diagnostic rows are not proof inputs. They are constructed outside the
        // controlled callback; only its Copy summary escapes, prepaid below.
        let mut input = original.clone();
        let mut rounds = Vec::new();
        for round in owner.history().rounds() {
            let current = subject(round.output());
            rounds.push(Round { ordinal: round.ordinal(), input: input.clone(), integer: subject(round.integer().owner()), changed: input != current, output: current.clone() });
            input = current;
        }
        let summary_storage = std::mem::size_of::<Checks>();
        budget.reserve_storage(summary_storage).map_err(|e| e.to_string())?;
        let checks = owner.with_policy_checks_v1(&mut budget, |view, budget| {
            let source = view.original_metadata(budget)?;
            assert!(std::ptr::eq(source.inventory(budget)?.owner(), owner.original_source().executable()));
            let spans = source.spans(budget)?;
            budget.charge_work(spans.len())?;
            let span_count = spans.len();
            let zero_spans = spans.iter().filter(|s| s.operations().is_empty()).count();
            let output = view.final_inventory(budget)?;
            assert!(std::ptr::eq(output.owner(), owner.output()));
            let lineage = view.lineage(budget)?;
            let definitions = lineage.original_definition_count(budget)?;
            let (mut elisions, mut substitutions) = (0, 0);
            for original in 0..definitions {
                let count = lineage.definition_descendant_count(original, budget)?;
                elisions += usize::from(count == 0);
                for n in 0..count {
                    substitutions += usize::from(lineage.definition_descendant(original, n, budget)?.kind == fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Substituted);
                }
            }
            let policies = view.policies(budget)?;
            assert!(std::ptr::eq(policies.owner(budget)?, owner.output()));
            let functions = policies.function_count(budget)?;
            let mut stages = 0;
            for n in 0..functions {
                assert!(policies.report(n, budget)?.is_clean());
                stages += policies.report(n, budget)?.pass_order().len();
                assert_eq!(policies.history(n, budget)?.function(), n);
            }
            assert!(!view.ranked_verification_is_complete() && !view.grants_artifact_or_launch_authority());
            Ok::<_, ProductionCanonicalScalarSourceErrorV1>(Checks { functions, stages, pending: policies.pending_obligations().iter().count(), spans: span_count, zero_spans, definitions, substitutions, elisions })
        }).map_err(|e| e.to_string())?;
        require(budget.storage() == owner_floor + summary_storage, "final source callback cleanup")?;
        drop(owner);
        budget.release_storage(owner_floor).map_err(|e| e.to_string())?;
        budget.release_storage(summary_storage).map_err(|e| e.to_string())?;
        Ok(Observation { actual_target: request.target, original, output, roots, rounds, before_operations, after_operations, checks, source_floor, additional, owner_floor, final_storage: budget.storage(), work: budget.work() })
    }).map_err(|e| e.to_string())?
}

struct NeutralCallbacks {
    request: NeutralRequest,
    count: usize,
    result: Option<Result<Observation, String>>,
}
impl Callbacks for NeutralCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.count += 1;
        self.result = Some(if self.count == 1 {
            observe(tcx, &self.request)
        } else {
            Err("duplicate compiler callback".into())
        });
        Compilation::Stop
    }
}
#[test]
#[ignore = "strict consuming subprocess helper; missing parent request is failure"]
fn canonical_scalar_source_child() {
    let path =
        |key| PathBuf::from(env::var_os(key).expect("strict neutral child requires parent paths"));
    let request: NeutralRequest = read_json(&path(REQUEST_ENV), INPUT_CAP).unwrap();
    let invocation: Invocation = read_json(&path(ARGS_ENV), INPUT_CAP).unwrap();
    let result_path = path(RESULT_ENV);
    assert!(!result_path.exists());
    let mut callbacks = NeutralCallbacks {
        request: request.clone(),
        count: 0,
        result: None,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request(&request, &invocation)?;
        rustc_driver::run_compiler(&invocation.executed, &mut callbacks);
        require(callbacks.count == 1, "exactly one actual compiler callback")?;
        let row = callbacks.result.take().ok_or("missing callback")??;
        check_request(&request, &invocation)?;
        validate(&request, &row)?;
        Ok(row)
    }))
    .unwrap_or_else(|_| Err("compiler or callback panicked".into()));
    let success = result.is_ok();
    let report = Report {
        request,
        callbacks: callbacks.count,
        result,
    };
    if success {
        audit_successful_protocol(&report.request, &invocation, &report);
    }
    write_json(&result_path, &report, REPORT_CAP).unwrap();
    assert!(success, "strict canonical scalar source: {report:?}");
}
fn decode(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    expected: &NeutralRequest,
) -> Result<Observation, String> {
    require(status == Some(0), "exact zero child exit required")?;
    let bytes = bytes.ok_or("missing fresh report")?;
    require(bytes.len() <= REPORT_CAP, "oversized report")?;
    let report: Report = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    require(
        report.request == *expected && report.callbacks == 1,
        "stale/foreign request or callback count",
    )?;
    let row = report.result?;
    validate(expected, &row)?;
    Ok(row)
}

fn audit_successful_protocol(request: &NeutralRequest, invocation: &Invocation, report: &Report) {
    // Every negative starts from this actual accepted compiler observation.
    check_request(request, invocation).unwrap();
    assert!(report.result.is_ok());
    let bytes = serde_json::to_vec(report).unwrap();
    decode(Some(0), Some(&bytes), request).unwrap();
    for change in 0..7 {
        let mut changed = request.clone();
        match change {
            0 => changed.source[3].sha256[0] ^= 1,
            1 => changed.schema += 1,
            2 => changed.run_id.clear(),
            3 => changed.cwd.push("foreign-cwd"),
            4 => changed.captured[0] ^= 1,
            5 => changed.executed[0] ^= 1,
            _ => {
                changed.case = match changed.case {
                    NeutralCase::NormalNoop => NeutralCase::MutatingOpt0,
                    NeutralCase::MutatingOpt0 => NeutralCase::NormalNoop,
                }
            }
        }
        assert!(
            check_request(&changed, invocation).is_err(),
            "request mutation {change}"
        );
    }
    let mut changed = Invocation {
        captured: invocation.captured.clone(),
        executed: invocation.executed.clone(),
    };
    changed.executed.push("-Cdebuginfo=0".into());
    let mut changed_request = request.clone();
    changed_request.executed = args_hash(&changed.executed);
    assert!(check_request(&changed_request, &changed).is_err());

    let other_target = match request.target {
        Target::Gfx942 => Target::Gfx950,
        Target::Gfx950 => Target::Gfx942,
    };
    for (option, value) in [
        ("--crate-name", "foreign_fixture"),
        ("-Ctarget-cpu", other_target.cpu()),
    ] {
        let mut captured = invocation.captured.clone();
        let index = captured
            .iter()
            .position(|arg| arg == option || arg.starts_with(&format!("{option}=")))
            .unwrap();
        if captured[index] == option {
            captured[index + 1] = value.into();
        } else {
            captured[index] = format!("{option}={value}");
        }
        let changed = Invocation {
            executed: executed(&captured, request.case).unwrap(),
            captured,
        };
        let mut changed_request = request.clone();
        changed_request.captured = args_hash(&changed.captured);
        changed_request.executed = args_hash(&changed.executed);
        assert!(check_request(&changed_request, &changed).is_err());
    }

    for change in 0..6 {
        let mut changed = serde_json::to_value(report).unwrap();
        match change {
            0 => {
                changed["request"]["run_id"] =
                    serde_json::json!(format!("{}-foreign", request.run_id))
            }
            1 => changed["callbacks"] = serde_json::json!(0),
            2 => changed["callbacks"] = serde_json::json!(2),
            3 => {
                changed["result"]["Ok"]["actual_target"] =
                    serde_json::to_value(other_target).unwrap()
            }
            4 => changed["target_bound_stage"] = serde_json::json!(true),
            _ => changed["result"] = serde_json::json!({"Err": "source refusal"}),
        }
        assert!(
            decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                request
            )
            .is_err(),
            "report mutation {change}"
        );
    }
    for status in [None, Some(1), Some(101)] {
        assert!(decode(status, Some(&bytes), request).is_err());
    }
    assert!(decode(Some(0), None, request).is_err());
    assert!(decode(Some(0), Some(b"{"), request).is_err());
    let mut oversized = bytes.clone();
    oversized.resize(REPORT_CAP + 1, b' ');
    assert!(serde_json::from_slice::<Report>(&oversized).is_ok());
    assert!(decode(Some(0), Some(&oversized), request).is_err());
    decode(Some(0), Some(&bytes), request).unwrap();
}

fn fixture(case: NeutralCase, target: Target) -> corpus::Fixture {
    let source = stamps().unwrap();
    let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    corpus::Fixture {
        fixture_id: format!("canonical-neutral-{case:?}-{}", target.cpu()),
        target: target.cpu().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hex(source[1].sha256),
            package_manifest: format!("{BASE}/Cargo.toml"),
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hex(source[0].sha256),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/scalar_fixed_point.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec!["scalar-fixed-point".into()],
            kernel_symbols: vec![case.root().into()],
        },
    }
}
fn qualify(case: NeutralCase) {
    let scratch = crate::test_temp_dir::TestTempDir::create("canonical-neutral-source");
    for target in [Target::Gfx942, Target::Gfx950] {
        let directory = scratch.path().join(target.cpu());
        std::fs::create_dir(&directory).unwrap();
        let captured = corpus_cargo::capture(
            &workspace(),
            &fixture(case, target),
            &directory,
            &scratch.path().join("cargo-target"),
        )
        .unwrap();
        let invocation = Invocation {
            executed: executed(&captured.args, case).unwrap(),
            captured: captured.args,
        };
        let request = NeutralRequest {
            schema: 1,
            run_id: format!("{}-{case:?}-{}", directory.display(), std::process::id()),
            case,
            target,
            captured: args_hash(&invocation.captured),
            executed: args_hash(&invocation.executed),
            cwd: captured.cwd.clone(),
            source: stamps().unwrap(),
        };
        let (request_path, args_path, result_path) = (
            directory.join("request.json"),
            directory.join("invocation.json"),
            directory.join("report.json"),
        );
        write_json(&request_path, &request, INPUT_CAP).unwrap();
        write_json(&args_path, &invocation, INPUT_CAP).unwrap();
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .env_clear()
            .envs(captured.environment.iter().cloned())
            .current_dir(&captured.cwd);
        for (key, _) in &captured.environment {
            if key.to_string_lossy().starts_with("FE2O3_TEST_") {
                command.env_remove(key);
            }
        }
        command
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER");
        progress::clear_inherited_jobserver(&mut command);
        census::configure(&mut command, None);
        simulation::configure_child(&mut command, None);
        command
            .env(REQUEST_ENV, &request_path)
            .env(ARGS_ENV, &args_path)
            .env(RESULT_ENV, &result_path)
            .args(["--exact", CHILD_NAME, "--ignored", "--nocapture"]);
        let output = command.output().unwrap();
        let bytes = std::fs::File::open(&result_path).ok().map(|file| {
            let mut bytes = Vec::new();
            file.take(REPORT_CAP as u64 + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        });
        let observed =
            decode(output.status.code(), bytes.as_deref(), &request).unwrap_or_else(|e| {
                panic!(
                    "neutral {case:?}/{}: {e}\n{}",
                    target.cpu(),
                    corpus_cargo::diagnostics(&output)
                )
            });
        assert_eq!(request.source, stamps().unwrap());
        println!(
            "CANONICAL_NEUTRAL_SOURCE {} {case:?} callbacks=1 roots={} rounds={} stages={} pending=19",
            target.cpu(),
            observed.roots.len(),
            observed.rounds.len(),
            observed.checks.stages
        );
    }
}
#[test]
#[ignore = "actual consuming pre-ranked mutating source, both collection profiles"]
fn ordinary_rust_canonical_scalar_neutral_mutating_both_profiles() {
    qualify(NeutralCase::MutatingOpt0);
}
#[test]
#[ignore = "actual consuming pre-ranked no-op source, both collection profiles"]
fn ordinary_rust_canonical_scalar_neutral_noop_both_profiles() {
    qualify(NeutralCase::NormalNoop);
}

#[test]
fn canonical_scalar_source_protocol_refuses_invalid_invocation_and_failed_child_report() {
    // A negative wire sample, never a manufactured source owner or positive gate.
    let invocation = Invocation {
        captured: vec![
            "rustc".into(),
            "-Coverflow-checks=on".into(),
            "-Ctarget-cpu=gfx942".into(),
            "--crate-name=fe2o3_production_extraction_fixture".into(),
            stamps().unwrap()[2].path.to_str().unwrap().into(),
        ],
        executed: vec![],
    };
    let request = NeutralRequest {
        schema: 1,
        run_id: "negative-wire-only".into(),
        case: NeutralCase::NormalNoop,
        target: Target::Gfx942,
        captured: args_hash(&invocation.captured),
        executed: [0; 32],
        cwd: env::current_dir().unwrap(),
        source: stamps().unwrap(),
    };
    assert!(check_request(&request, &invocation).is_err());
    let failure = Report {
        request: request.clone(),
        callbacks: 1,
        result: Err("exact source refusal".into()),
    };
    let bytes = serde_json::to_vec(&failure).unwrap();
    for code in [None, Some(1), Some(101)] {
        assert!(decode(code, Some(&bytes), &request).is_err());
    }
    assert!(decode(Some(0), None, &request).is_err());
    assert!(decode(Some(0), Some(&bytes), &request).is_err());
}

mod private_call_history {
    include!("production_rustc_driver_canonical_private_call_v1_tests.rs");
}
