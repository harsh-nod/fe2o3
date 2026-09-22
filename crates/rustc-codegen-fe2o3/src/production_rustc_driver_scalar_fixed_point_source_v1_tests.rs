//! Distinct pre-ranked trap and admitted B-to-scalar source component gates.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, BinaryOp,
    CheckedBinaryOperator as Checked, Constant, Function, FunctionRole, LaunchDomain, LaunchExtent,
    Module, Operation, OperationKind as Kind, ScalarType, Terminator, Type, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner, VerifiedCanonicalKernelIrV12, WorkgroupSize,
};
use fe2o3_kernel_opt::prepare_checked_scalar_fixed_point_v1 as prepare;
use fe2o3_kir_sim as sim;
use std::{
    cell::RefCell,
    collections::BTreeSet,
    io::{Read, Write},
    rc::Rc,
};
#[path = "production_rustc_driver_scalar_fixed_point_component_v1_tests.rs"]
mod component;
#[path = "production_rustc_driver_scalar_fixed_point_assertion_v1_tests.rs"]
mod component_assertion;
#[path = "production_rustc_driver_scalar_fixed_point_simulation_v1_tests.rs"]
mod component_sim;
#[path = "production_rustc_driver_scalar_fixed_point_target_v1_tests.rs"]
mod component_target;
use component::observe;
use component_sim::{SimRow, scenarios, simulate_matrix};

const REQUEST_ENV: &str = "FE2O3_TEST_SCALAR_SOURCE_REQUEST_V1";
const ARGS_ENV: &str = "FE2O3_TEST_SCALAR_SOURCE_ARGS_V1";
const RESULT_ENV: &str = "FE2O3_TEST_SCALAR_SOURCE_RESULT_V1";
const CHILD_NAME: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::scalar_fixed_point_source::scalar_fixed_point_source_child";
const CFG: &str = "fe2o3_scalar_fixed_point_noop";
const SAFE_CFG: &str = "fe2o3_scalar_fixed_point_safe";
const INPUT_CAP: usize = 1024 * 1024;
const REPORT_CAP: usize = 8 * 1024 * 1024;
const OBSERVATION_CAP: usize = 100_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum ScalarCase {
    PreRankedCheckedOpt0,
    AdmittedCheckedOpt0,
    NormalNoop,
}
impl ScalarCase {
    fn roots(self) -> &'static [&'static str] {
        match self {
            Self::PreRankedCheckedOpt0 => {
                &["scalar_checked_identity", "scalar_effect_then_overflow"]
            }
            Self::AdmittedCheckedOpt0 => &["scalar_checked_identity"],
            Self::NormalNoop => &["scalar_noop_control"],
        }
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ScalarRequestV1 {
    schema: u16,
    run_id: String,
    case: ScalarCase,
    target: Target,
    captured_args_sha256: [u8; 32],
    executed_args_sha256: [u8; 32],
    cwd: PathBuf,
    source: [FileStamp; 4],
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScalarInvocationV1 {
    captured_args: Vec<String>,
    executed_args: Vec<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
enum Phase {
    Request,
    Rustc,
    Collect,
    Ranked,
    Bound,
    Scalar,
    Replay,
    Abi,
    Sim,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    phase: Phase,
    detail: String,
}
type ResultV1<T> = Result<T, Failure>;
fn failure(phase: Phase, detail: impl std::fmt::Display) -> Failure {
    Failure {
        phase,
        detail: detail.to_string(),
    }
}
fn require(ok: bool, phase: Phase, detail: &str) -> ResultV1<()> {
    if ok {
        Ok(())
    } else {
        Err(failure(phase, detail))
    }
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScalarReportV1 {
    request: ScalarRequestV1,
    callback_count: usize,
    result: ResultV1<Observation>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    digest: [u8; 32],
    bytes: usize,
}
fn subject(owner: &Owner) -> Subject {
    Subject {
        digest: *owner.canonical().identity().digest(),
        bytes: owner.canonical().canonical_bytes().len(),
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RootRow {
    name: String,
    source_function: [u8; 32],
    source_body: [u8; 32],
    entry: String,
    abi: String,
    launch: String,
    before: [usize; 5],
    after: [usize; 5],
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceRootRow {
    name: String,
    function: [u8; 32],
    body: [u8; 32],
    entry: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RoundRow {
    ordinal: u16,
    input: Subject,
    integer: Subject,
    output: Subject,
    integer_execution: [u8; 32],
    scalar_execution: [u8; 32],
    changed: bool,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum SourceStage {
    Bound {
        original: Subject,
        erased: Option<[u8; 32]>,
    },
    PreRanked {
        assertion: component_assertion::AssertionRow,
        required_proof_refused: bool,
    },
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    actual_target: Target,
    semantic: Subject,
    stage: SourceStage,
    input: Subject,
    output: Subject,
    source_roots: Vec<SourceRootRow>,
    roots: Vec<RootRow>,
    rounds: Vec<RoundRow>,
    execution: [u8; 32],
    terminal_round: usize,
    replay_work: usize,
    retained_floor: usize,
    history_storage: usize,
    idempotent_storage: usize,
    peak_storage: usize,
    component_work: usize,
    final_storage: usize,
    simulations: Vec<SimRow>,
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
fn stamps() -> ResultV1<[FileStamp; 4]> {
    let base = workspace();
    let paths = [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/scalar_fixed_point.rs"),
    ];
    let rows = paths
        .into_iter()
        .map(|path| {
            let path = base
                .join(path)
                .canonicalize()
                .map_err(|e| failure(Phase::Request, e))?;
            let bytes = std::fs::read(&path).map_err(|e| failure(Phase::Request, e))?;
            Ok(FileStamp {
                path,
                sha256: digest(&bytes),
            })
        })
        .collect::<ResultV1<Vec<_>>>()?;
    rows.try_into()
        .map_err(|_| failure(Phase::Request, "source roster extent"))
}
fn args_hash(args: &[String]) -> [u8; 32] {
    digest(&serde_json::to_vec(args).unwrap())
}
fn executed(captured: &[String], case: ScalarCase) -> ResultV1<Vec<String>> {
    require_canonical_overflow_checks_v1(captured).map_err(|e| failure(Phase::Request, e))?;
    require(
        !captured.iter().any(|arg| {
            arg.contains("fe2o3_scalar_fixed_point_")
                || arg.contains("mir-opt-level")
                || arg.contains("inline-mir")
        }),
        Phase::Request,
        "preexisting test cfg/MIR override",
    )?;
    let mut args = captured.to_vec();
    args.push(format!("--check-cfg=cfg({CFG})"));
    args.push(format!("--check-cfg=cfg({SAFE_CFG})"));
    match case {
        ScalarCase::PreRankedCheckedOpt0 => args.push("-Zmir-opt-level=0".into()),
        ScalarCase::AdmittedCheckedOpt0 => {
            args.push("-Zmir-opt-level=0".into());
            args.push(format!("--cfg={SAFE_CFG}"));
        }
        ScalarCase::NormalNoop => args.push(format!("--cfg={CFG}")),
    }
    Ok(args)
}
fn option_values<'a>(args: &'a [String], prefix: &str) -> Vec<&'a str> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            if arg == prefix {
                args.get(index + 1).map(String::as_str)
            } else {
                arg.strip_prefix(&format!("{prefix}="))
            }
        })
        .collect()
}
fn check_request(request: &ScalarRequestV1, invocation: &ScalarInvocationV1) -> ResultV1<()> {
    require(
        request.schema == 3
            && !request.run_id.is_empty()
            && request.run_id.len() <= 256
            && request.source == stamps()?
            && request.cwd == env::current_dir().map_err(|e| failure(Phase::Request, e))?
            && request
                .source
                .iter()
                .map(|row| &row.path)
                .collect::<BTreeSet<_>>()
                .len()
                == 4
            && request.captured_args_sha256 == args_hash(&invocation.captured_args)
            && request.executed_args_sha256 == args_hash(&invocation.executed_args)
            && invocation.executed_args == executed(&invocation.captured_args, request.case)?,
        Phase::Request,
        "request/source/cwd/exact argv binding",
    )?;
    let args = &invocation.captured_args;
    require(
        option_values(args, "-Ctarget-cpu") == [request.target.cpu()]
            && option_values(args, "--crate-name") == ["fe2o3_production_extraction_fixture"]
            && args
                .iter()
                .filter(|arg| {
                    request.cwd.join(arg).canonicalize().ok()
                        == Some(request.source[2].path.clone())
                })
                .count()
                == 1,
        Phase::Request,
        "captured target/crate/source entry",
    )
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path, cap: usize) -> ResultV1<T> {
    let file = std::fs::File::open(path).map_err(|e| failure(Phase::Request, e))?;
    let mut bytes = Vec::new();
    file.take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| failure(Phase::Request, e))?;
    require(bytes.len() <= cap, Phase::Request, "bounded JSON exceeded")?;
    serde_json::from_slice(&bytes).map_err(|e| failure(Phase::Request, e))
}
fn write_json(path: &Path, value: &impl Serialize, cap: usize) -> ResultV1<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| failure(Phase::Request, e))?;
    require(bytes.len() <= cap, Phase::Request, "bounded JSON exceeded")?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|e| failure(Phase::Request, e))
}
fn operations(function: &Function) -> impl Iterator<Item = &Operation> {
    function
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
}
fn literal(function: &Function, id: ValueId) -> Option<u32> {
    operations(function).find_map(|op| match &op.kind {
        Kind::Constant(Constant::U32(value)) if op.results.len() == 1 && op.results[0].id == id => {
            Some(*value)
        }
        _ => None,
    })
}
fn is_trap(kind: &Kind) -> bool {
    matches!(kind, Kind::Call { callee, arguments } if matches!(
        AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments), Some(AmdGpuDiagnosticOperation::Trap)))
}
fn counts(function: &Function) -> ResultV1<[usize; 5]> {
    let mut counts = [0; 5];
    for op in operations(function) {
        if is_trap(&op.kind) {
            counts[4] += 1;
        }
        if matches!(op.kind, Kind::Store { .. } | Kind::GuardedStore { .. }) {
            counts[3] += 1;
        }
        let Kind::Binary {
            op: BinaryOp::Checked(operator),
            lhs,
            rhs,
        } = &op.kind
        else {
            continue;
        };
        let [value, overflow] = op.results.as_slice() else {
            return Err(failure(Phase::Bound, "checked pair missing"));
        };
        require(
            value.ty == Type::Scalar(ScalarType::U32)
                && overflow.ty == Type::Scalar(ScalarType::Bool),
            Phase::Bound,
            "actual checked U32/Bool result pair",
        )?;
        let neutral = match operator {
            Checked::Add => Some(0),
            Checked::Multiply => Some(1),
            _ => None,
        };
        let index = if neutral.is_some()
            && (literal(function, *lhs) == neutral || literal(function, *rhs) == neutral)
        {
            if *operator == Checked::Add { 0 } else { 1 }
        } else {
            2
        };
        require(
            operations(function).any(|op| op.kind.operands().contains(&value.id))
                || function.body.iter().flat_map(|body| &body.blocks).any(|b| {
                    b.terminator
                        .as_ref()
                        .is_some_and(|t| t.operands().contains(&value.id))
                }),
            Phase::Bound,
            "checked value has no actual use",
        )?;
        // Follow actual SSA dependences through assertion comparisons to a terminator.
        let mut dependent = BTreeSet::from([overflow.id]);
        loop {
            let before = dependent.len();
            for op in operations(function) {
                if op.kind.operands().iter().any(|id| dependent.contains(id)) {
                    dependent.extend(op.results.iter().map(|result| result.id));
                }
            }
            if dependent.len() == before {
                break;
            }
        }
        require(
            function
                .body
                .iter()
                .flat_map(|body| &body.blocks)
                .any(|block| {
                    block.terminator.as_ref().is_some_and(|t| match t {
                        Terminator::ConditionalBranch { condition, .. } => {
                            dependent.contains(condition)
                        }
                        Terminator::Switch { selector, .. }
                        | Terminator::IntegerSwitch { selector, .. } => {
                            dependent.contains(selector)
                        }
                        _ => false,
                    })
                }),
            Phase::Bound,
            "checked overflow lacks real assertion/control use",
        )?;
        counts[index] += 1;
    }
    Ok(counts)
}
fn check_counts(
    case: ScalarCase,
    name: &str,
    before: [usize; 5],
    after: [usize; 5],
) -> ResultV1<()> {
    require(
        match case {
            ScalarCase::NormalNoop => before == [0; 5] && after == [0; 5],
            ScalarCase::PreRankedCheckedOpt0 | ScalarCase::AdmittedCheckedOpt0
                if name == case.roots()[0] =>
            {
                before[0] > 0
                    && before[1] > 0
                    && before[3] > 0
                    && before[4] > 0
                    && after[0] == 0
                    && after[1] == 0
                    && after[3] > 0
            }
            ScalarCase::PreRankedCheckedOpt0 => {
                before[2] == 1
                    && after[2] == 1
                    && before[3] == 2
                    && after[3] == 2
                    && before[4] > 0
                    && after[4] > 0
            }
            ScalarCase::AdmittedCheckedOpt0 => false,
        },
        Phase::Bound,
        "nonvacuous source occurrences/control",
    )
}
fn check_root_order(source: &[SourceRootRow], rows: &[RootRow], case: ScalarCase) -> ResultV1<()> {
    require(
        source.len() == case.roots().len()
            && source
                .iter()
                .map(|r| r.name.as_str())
                .collect::<BTreeSet<_>>()
                == case.roots().iter().copied().collect()
            && source.len() == rows.len()
            && source.iter().zip(rows).all(|(source, row)| {
                row.name == source.name
                    && row.source_function == source.function
                    && row.source_body == source.body
                    && row.entry == source.entry
                    && row.source_function != [0; 32]
                    && row.source_body != [0; 32]
                    && !row.entry.is_empty()
                    && !row.abi.is_empty()
                    && !row.launch.is_empty()
            }),
        Phase::Abi,
        "exact ordered source/report root identities",
    )
}
fn validate(request: &ScalarRequestV1, row: &Observation) -> ResultV1<()> {
    require(
        row.actual_target == request.target
            && row.semantic.digest != [0; 32]
            && [&row.semantic, &row.input, &row.output]
                .iter()
                .all(|s| s.bytes > 0 && s.digest != [0; 32])
            && row.execution != [0; 32]
            && row.retained_floor > 0
            && row.history_storage > 0
            && row.idempotent_storage > 0
            && row.final_storage == 0
            && row.replay_work > 0
            && row.component_work >= row.replay_work
            && row
                .retained_floor
                .checked_add(row.history_storage)
                .and_then(|n| n.checked_add(row.idempotent_storage))
                .is_some_and(|n| row.peak_storage >= n),
        Phase::Replay,
        "observation identities/paid resources",
    )?;
    require(
        match (&row.stage, request.case) {
            (
                SourceStage::PreRanked {
                    assertion,
                    required_proof_refused: true,
                },
                ScalarCase::PreRankedCheckedOpt0,
            ) => assertion.valid() && assertion.input == row.input,
            (
                SourceStage::Bound { original, .. },
                ScalarCase::AdmittedCheckedOpt0 | ScalarCase::NormalNoop,
            ) => original.bytes > 0 && original.digest != [0; 32],
            _ => false,
        },
        Phase::Ranked,
        "exact stage tag/source assertion/required-proof outcome",
    )?;
    if let SourceStage::PreRanked { assertion, .. } = &row.stage {
        assertion.check_file(&request.source[3])?;
    }
    check_root_order(&row.source_roots, &row.roots, request.case)?;
    require(
        !row.rounds.is_empty()
            && row.rounds.len() <= fe2o3_kernel_opt::SCALAR_FIXED_POINT_MAX_ROUNDS_V1
            && row.terminal_round == row.rounds.len() - 1,
        Phase::Replay,
        "observation terminal extent",
    )?;
    let mut input = &row.input;
    for (ordinal, round) in row.rounds.iter().enumerate() {
        require(
            round.ordinal as usize == ordinal
                && &round.input == input
                && round.integer.bytes > 0
                && round.integer.digest != [0; 32]
                && round.output.bytes > 0
                && round.output.digest != [0; 32]
                && round.integer_execution != [0; 32]
                && round.scalar_execution != [0; 32]
                && round.changed == (round.input != round.output)
                && round.changed == (ordinal != row.terminal_round),
            Phase::Replay,
            "observation complete adjacent rounds",
        )?;
        input = &round.output;
    }
    require(
        input == &row.output
            && match request.case {
                ScalarCase::PreRankedCheckedOpt0 | ScalarCase::AdmittedCheckedOpt0 => {
                    row.input != row.output && row.rounds.len() >= 2
                }
                ScalarCase::NormalNoop => row.input == row.output && row.rounds.len() == 1,
            },
        Phase::Scalar,
        "vacuous positive/changing no-op/wrong final subject",
    )?;
    for root in &row.roots {
        check_counts(request.case, &root.name, root.before, root.after)?;
    }
    require(
        row.simulations.len() == scenarios(request.case).len(),
        Phase::Sim,
        "complete SIM roster",
    )?;
    for (actual, expected) in row.simulations.iter().zip(scenarios(request.case)) {
        component_sim::validate_row(actual, &expected)?;
        if let SourceStage::PreRanked { assertion, .. } = &row.stage {
            component_sim::validate_assertion_row(actual, assertion, &row.input, &row.output)?;
        }
    }
    Ok(())
}
struct ScalarCallbacks {
    request: ScalarRequestV1,
    count: usize,
    result: Option<ResultV1<Observation>>,
}
impl Callbacks for ScalarCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.count += 1;
        self.result = Some(if self.count == 1 {
            observe(tcx, &self.request)
        } else {
            Err(failure(Phase::Rustc, "duplicate actual compiler callback"))
        });
        Compilation::Stop
    }
}
#[test]
#[ignore = "strict subprocess helper; missing parent request is failure"]
fn scalar_fixed_point_source_child() {
    let path =
        |key| PathBuf::from(env::var_os(key).expect("strict scalar child requires parent paths"));
    let request: ScalarRequestV1 = read_json(&path(REQUEST_ENV), INPUT_CAP).unwrap();
    let invocation: ScalarInvocationV1 = read_json(&path(ARGS_ENV), INPUT_CAP).unwrap();
    let result_path = path(RESULT_ENV);
    assert!(!result_path.exists(), "result must be fresh");
    let mut callbacks = ScalarCallbacks {
        request: request.clone(),
        count: 0,
        result: None,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request(&request, &invocation)?;
        rustc_driver::run_compiler(&invocation.executed_args, &mut callbacks);
        require(
            callbacks.count == 1,
            Phase::Rustc,
            "exactly one real callback required",
        )?;
        let observation = callbacks
            .result
            .take()
            .ok_or_else(|| failure(Phase::Rustc, "missing callback result"))??;
        check_request(&request, &invocation)?;
        validate(&request, &observation)?;
        Ok(observation)
    }))
    .unwrap_or_else(|_| Err(failure(Phase::Rustc, "rustc or callback panicked")));
    let succeeded = result.is_ok();
    let report = ScalarReportV1 {
        request,
        callback_count: callbacks.count,
        result,
    };
    write_json(&result_path, &report, REPORT_CAP).unwrap();
    assert!(succeeded, "strict scalar source gate: {report:?}");
}
fn decode(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    expected: &ScalarRequestV1,
) -> ResultV1<Observation> {
    require(
        status == Some(0),
        Phase::Rustc,
        "child exit must be exactly zero",
    )?;
    let bytes = bytes.ok_or_else(|| failure(Phase::Request, "missing fresh report"))?;
    require(
        bytes.len() <= REPORT_CAP,
        Phase::Request,
        "oversized report",
    )?;
    let report: ScalarReportV1 =
        serde_json::from_slice(bytes).map_err(|e| failure(Phase::Request, e))?;
    require(
        &report.request == expected && report.callback_count == 1,
        Phase::Request,
        "foreign request/callback count",
    )?;
    let observation = report.result?;
    validate(expected, &observation)?;
    Ok(observation)
}
fn fixture(case: ScalarCase, target: Target) -> corpus::Fixture {
    let stamps = stamps().unwrap();
    let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    corpus::Fixture {
        fixture_id: format!("scalar-{case:?}-{}", target.cpu()),
        target: target.cpu().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hex(stamps[1].sha256),
            package_manifest: format!("{BASE}/Cargo.toml"),
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hex(stamps[0].sha256),
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
            kernel_symbols: case.roots().iter().map(|s| (*s).into()).collect(),
        },
    }
}
fn qualify(case: ScalarCase) {
    let scratch = crate::test_temp_dir::TestTempDir::create("scalar-b-component");
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
        let invocation = ScalarInvocationV1 {
            executed_args: executed(&captured.args, case).unwrap(),
            captured_args: captured.args,
        };
        let request = ScalarRequestV1 {
            schema: 3,
            run_id: format!("{}-{case:?}-{}", directory.display(), std::process::id()),
            case,
            target,
            captured_args_sha256: args_hash(&invocation.captured_args),
            executed_args_sha256: args_hash(&invocation.executed_args),
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
        assert!(!result_path.exists());
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
                    "scalar {case:?}/{}: {e:?}\n{}",
                    target.cpu(),
                    corpus_cargo::diagnostics(&output)
                )
            });
        assert_eq!(request.source, stamps().unwrap());
        println!(
            "SCALAR_SOURCE_COMPONENT {} {case:?} callbacks=1 roots={} scenarios={} sim_executions={}",
            target.cpu(),
            observed.roots.len(),
            observed.simulations.len(),
            observed.simulations.len() * 4
        );
    }
}
#[test]
#[ignore = "actual pre-ranked trap SIM then same-stage required-proof refusal"]
fn ordinary_rust_scalar_pre_ranked_trap_and_required_proof_refusal_both_profiles() {
    qualify(ScalarCase::PreRankedCheckedOpt0);
}
#[test]
#[ignore = "actual safely admitted opt0 B-to-scalar source on both profiles"]
fn ordinary_rust_scalar_fixed_point_b_component_admitted_opt0_both_profiles() {
    qualify(ScalarCase::AdmittedCheckedOpt0);
}
#[test]
#[ignore = "actual normal-MIR source B-to-scalar no-op qualification on both profiles"]
fn ordinary_rust_scalar_fixed_point_b_component_normal_noop_both_profiles() {
    qualify(ScalarCase::NormalNoop);
}

fn protocol_sample() -> (ScalarRequestV1, ScalarInvocationV1, Observation) {
    let captured_args = vec![
        "rustc".into(),
        "-Coverflow-checks=on".into(),
        "-Ctarget-cpu=gfx942".into(),
        "--crate-name=fe2o3_production_extraction_fixture".into(),
        stamps().unwrap()[2].path.to_str().unwrap().into(),
    ];
    let invocation = ScalarInvocationV1 {
        executed_args: executed(&captured_args, ScalarCase::NormalNoop).unwrap(),
        captured_args,
    };
    let request = ScalarRequestV1 {
        schema: 3,
        run_id: "synthetic-protocol-negative-test-only".into(),
        case: ScalarCase::NormalNoop,
        target: Target::Gfx942,
        captured_args_sha256: args_hash(&invocation.captured_args),
        executed_args_sha256: args_hash(&invocation.executed_args),
        cwd: env::current_dir().unwrap(),
        source: stamps().unwrap(),
    };
    let subject = Subject {
        digest: [1; 32],
        bytes: 10,
    };
    // A diagnostic wire sample, not a manufactured compiler owner or source proof.
    let observation = serde_json::from_value(serde_json::json!({
        "actual_target": request.target, "semantic": subject,
        "stage": {"kind":"Bound", "original": subject, "erased": null},
        "input": subject, "output": subject,
        "source_roots": [{ "name": request.case.roots()[0], "function": ([1_u8; 32]),
            "body": ([2_u8; 32]), "entry": "synthetic" }],
        "roots": [{ "name": request.case.roots()[0], "source_function": ([1_u8; 32]),
            "source_body": ([2_u8; 32]), "entry": "synthetic", "abi": "()",
            "launch": "synthetic", "before": ([0; 5]), "after": ([0; 5]) }],
        "rounds": [{ "ordinal": 0, "input": subject, "integer": subject, "output": subject,
            "integer_execution": ([3_u8; 32]), "scalar_execution": ([4_u8; 32]), "changed": false }],
        "execution": ([5_u8; 32]), "terminal_round": 0, "replay_work": 1,
        "retained_floor": 1, "history_storage": 2, "idempotent_storage": 3,
        "peak_storage": 6, "component_work": 1, "final_storage": 0,
        "simulations": component_sim::protocol_rows(request.case)
    })).unwrap();
    (request, invocation, observation)
}
#[test]
fn scalar_source_matrix_is_exact_two_profiles_positive_and_noop() {
    assert_eq!(scenarios(ScalarCase::PreRankedCheckedOpt0).len(), 60);
    assert_eq!(scenarios(ScalarCase::AdmittedCheckedOpt0).len(), 40);
    assert_eq!(scenarios(ScalarCase::NormalNoop).len(), 1);
    assert_eq!((60 + 1) * 2 * 4, 488);
    assert_eq!((60 + 40 + 1) * 2 * 4, 808);
    for case in [
        ScalarCase::PreRankedCheckedOpt0,
        ScalarCase::AdmittedCheckedOpt0,
        ScalarCase::NormalNoop,
    ] {
        let args = executed(&["rustc".into(), "-Coverflow-checks=on".into()], case).unwrap();
        assert_eq!(
            args.len(),
            if case == ScalarCase::AdmittedCheckedOpt0 {
                6
            } else {
                5
            }
        );
        assert_eq!(args[2], format!("--check-cfg=cfg({CFG})"));
        assert_eq!(args[3], format!("--check-cfg=cfg({SAFE_CFG})"));
        assert_eq!(
            args[4],
            if case == ScalarCase::NormalNoop {
                format!("--cfg={CFG}")
            } else {
                "-Zmir-opt-level=0".into()
            }
        );
        if case == ScalarCase::AdmittedCheckedOpt0 {
            assert_eq!(args[5], format!("--cfg={SAFE_CFG}"));
        }
    }
}
#[test]
fn scalar_source_request_rejects_changed_duplicate_foreign_source_argv_and_cfg() {
    let (request, invocation, _) = protocol_sample();
    check_request(&request, &invocation).unwrap();
    for mutation in 0..6 {
        let mut changed = request.clone();
        match mutation {
            0 => changed.source[0].sha256[0] ^= 1,
            1 => changed.source[1] = changed.source[0].clone(),
            2 => changed.source[3].path = changed.source[2].path.clone(),
            3 => changed.executed_args_sha256[0] ^= 1,
            4 => changed.target = Target::Gfx950,
            _ => changed.cwd.push("foreign"),
        }
        assert!(check_request(&changed, &invocation).is_err());
    }
    for flag in [
        format!("--cfg={CFG}"),
        format!("--cfg={SAFE_CFG}"),
        "--cfg=fe2o3_scalar_fixed_point_unknown".into(),
        "-Zmir-opt-level=2".into(),
        "-Zinline-mir=no".into(),
    ] {
        let mut args = invocation.captured_args.clone();
        args.push(flag);
        assert!(executed(&args, request.case).is_err());
    }
    let unknown = serde_json::json!({"captured_args": [], "executed_args": [], "extra": true});
    assert!(serde_json::from_value::<ScalarInvocationV1>(unknown).is_err());
}
#[test]
fn scalar_source_report_rejects_exit_missing_malformed_foreign_zero_or_duplicate_callback() {
    let (request, _, observation) = protocol_sample();
    let mut report = ScalarReportV1 {
        request: request.clone(),
        callback_count: 1,
        result: Ok(observation),
    };
    let bytes = serde_json::to_vec(&report).unwrap();
    decode(Some(0), Some(&bytes), &request).unwrap();
    for exit in [None, Some(1), Some(101)] {
        assert!(decode(exit, Some(&bytes), &request).is_err());
    }
    assert!(decode(Some(0), None, &request).is_err());
    assert!(decode(Some(0), Some(b"{"), &request).is_err());
    for count in [0, 2] {
        report.callback_count = count;
        assert!(
            decode(
                Some(0),
                Some(&serde_json::to_vec(&report).unwrap()),
                &request
            )
            .is_err()
        );
    }
    report.callback_count = 1;
    report.request.run_id.push_str("foreign");
    assert!(
        decode(
            Some(0),
            Some(&serde_json::to_vec(&report).unwrap()),
            &request
        )
        .is_err()
    );
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["unknown"] = true.into();
    assert!(
        decode(
            Some(0),
            Some(&serde_json::to_vec(&value).unwrap()),
            &request
        )
        .is_err()
    );
}
#[test]
fn scalar_source_observation_rejects_vacuous_positive_wrong_subject_and_changing_noop() {
    for mutation in 0..8 {
        let (mut request, _, mut row) = protocol_sample();
        match mutation {
            0 => request.case = ScalarCase::PreRankedCheckedOpt0,
            1 => row.output.digest[0] ^= 1,
            2 => row.rounds[0].changed = true,
            3 => row.rounds[0].input.digest[0] ^= 1,
            4 => row.roots[0].source_body = [0; 32],
            5 => row.final_storage = 1,
            6 => row.simulations.clear(),
            _ => row.peak_storage = 5,
        }
        assert!(validate(&request, &row).is_err());
    }
}

#[test]
fn scalar_source_protocol_keeps_pre_ranked_refusal_distinct_from_bound_success() {
    let (mut request, invocation, row) = protocol_sample();
    for schema in [1, 2] {
        request.schema = schema;
        assert!(check_request(&request, &invocation).is_err());
    }
    request.schema = 3;
    request.case = ScalarCase::PreRankedCheckedOpt0;
    let error = validate(&request, &row).unwrap_err();
    assert!(matches!(error.phase, Phase::Ranked));
    assert_eq!(
        error.detail,
        "exact stage tag/source assertion/required-proof outcome"
    );
    let mut value = serde_json::to_value(&row).unwrap();
    value["stage"]["kind"] = "PreRanked".into();
    assert!(serde_json::from_value::<Observation>(value.clone()).is_err());
    value["stage"]["kind"] = "Bound".into();
    value["stage"]["required_proof_refused"] = true.into();
    assert!(serde_json::from_value::<Observation>(value).is_err());
}

#[test]
fn scalar_source_root_order_is_positional_not_declaration_or_set_order() {
    let (_, _, row) = protocol_sample();
    let case = ScalarCase::PreRankedCheckedOpt0;
    let source = case
        .roots()
        .iter()
        .rev()
        .enumerate()
        .map(|(i, name)| SourceRootRow {
            name: (*name).into(),
            function: [i as u8 + 1; 32],
            body: [i as u8 + 3; 32],
            entry: format!("entry_{i}"),
        })
        .collect::<Vec<_>>();
    let rows = source
        .iter()
        .map(|source| RootRow {
            name: source.name.clone(),
            source_function: source.function,
            source_body: source.body,
            entry: source.entry.clone(),
            ..row.roots[0].clone()
        })
        .collect::<Vec<_>>();
    check_root_order(&source, &rows, case).unwrap();
    let mut extra = serde_json::to_value(&source[0]).unwrap();
    extra["extra"] = true.into();
    assert!(serde_json::from_value::<SourceRootRow>(extra).is_err());
    for mutation in 0..8 {
        let mut changed = rows.clone();
        match mutation {
            0 => changed.swap(0, 1),
            1 => {
                changed.pop();
            }
            2 => changed[1] = changed[0].clone(),
            3 => changed.push(changed[0].clone()),
            4 => changed[0].source_function[0] ^= 1,
            5 => changed[0].source_body[0] ^= 1,
            6 => changed[0].name = "foreign".into(),
            _ => changed[0].entry = "foreign".into(),
        }
        assert!(check_root_order(&source, &changed, case).is_err());
    }
}
