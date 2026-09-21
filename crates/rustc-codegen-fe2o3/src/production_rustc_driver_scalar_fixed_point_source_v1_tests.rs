//! Ordinary B-to-scalar component gate. No U/native/default-pipeline qualification.
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
#[path = "production_rustc_driver_scalar_fixed_point_simulation_v1_tests.rs"]
mod component_sim;
#[path = "production_rustc_driver_scalar_fixed_point_target_v1_tests.rs"]
mod component_target;
use component_sim::{SimRow, scenarios, simulate_matrix};

const REQUEST_ENV: &str = "FE2O3_TEST_SCALAR_SOURCE_REQUEST_V1";
const ARGS_ENV: &str = "FE2O3_TEST_SCALAR_SOURCE_ARGS_V1";
const RESULT_ENV: &str = "FE2O3_TEST_SCALAR_SOURCE_RESULT_V1";
const CHILD_NAME: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::scalar_fixed_point_source::scalar_fixed_point_source_child";
const CFG: &str = "fe2o3_scalar_fixed_point_noop";
const INPUT_CAP: usize = 1024 * 1024;
const REPORT_CAP: usize = 8 * 1024 * 1024;
const OBSERVATION_CAP: usize = 100_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum ScalarCase {
    CheckedOpt0,
    NormalNoop,
}
impl ScalarCase {
    fn roots(self) -> &'static [&'static str] {
        match self {
            Self::CheckedOpt0 => &["scalar_checked_identity", "scalar_effect_then_overflow"],
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
#[derive(Debug, Deserialize, Serialize)]
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
#[serde(deny_unknown_fields)]
struct Observation {
    actual_target: Target,
    semantic: Subject,
    original: Subject,
    erased: Option<[u8; 32]>,
    bound: Subject,
    output: Subject,
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
            arg.contains(CFG) || arg.contains("mir-opt-level") || arg.contains("inline-mir")
        }),
        Phase::Request,
        "preexisting test cfg/MIR override",
    )?;
    let mut args = captured.to_vec();
    args.push(format!("--check-cfg=cfg({CFG})"));
    args.push(match case {
        ScalarCase::CheckedOpt0 => "-Zmir-opt-level=0".into(),
        ScalarCase::NormalNoop => format!("--cfg={CFG}"),
    });
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
        request.schema == 1
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
            ScalarCase::CheckedOpt0 if name == case.roots()[0] => {
                before[0] > 0
                    && before[1] > 0
                    && before[3] > 0
                    && before[4] > 0
                    && after[0] == 0
                    && after[1] == 0
                    && after[3] > 0
            }
            ScalarCase::CheckedOpt0 => {
                before[2] == 1
                    && after[2] == 1
                    && before[3] == 2
                    && after[3] == 2
                    && before[4] > 0
                    && after[4] > 0
            }
        },
        Phase::Bound,
        "nonvacuous source occurrences/control",
    )
}
fn roots(stage: &Stage, output: &Owner, case: ScalarCase) -> ResultV1<Vec<RootRow>> {
    let source = census::roots(stage.semantic()).map_err(|e| failure(Phase::Abi, e))?;
    for root in stage.semantic().roots() {
        let entry = stage.semantic().functions()[root.index() as usize]
            .kernel_entry()
            .ok_or_else(|| failure(Phase::Abi, "source root entry absent"))?;
        let launch = entry
            .source_contract()
            .launch()
            .ok_or_else(|| failure(Phase::Abi, "source launch absent"))?;
        require(
            launch.required().map(|d| d.as_array()) == Some([64, 1, 1])
                && launch.maximum().map(|d| d.as_array()) == Some([64, 1, 1]),
            Phase::Abi,
            "actual source required/max launch",
        )?;
    }
    require(
        source
            .iter()
            .map(|r| r.name.as_str())
            .collect::<BTreeSet<_>>()
            == case.roots().iter().copied().collect(),
        Phase::Abi,
        "actual selected source roster",
    )?;
    let modules = component_target::checked_modules(stage, output, case.roots().len())?;
    let mut rows = Vec::new();
    for kernel in &modules[0].kernels {
        let root = source
            .iter()
            .find(|r| r.name == kernel.id.as_str())
            .ok_or_else(|| failure(Phase::Abi, "foreign root"))?;
        let entries = modules
            .iter()
            .map(|m| {
                m.function(&kernel.entry)
                    .ok_or_else(|| failure(Phase::Abi, "entry missing"))
            })
            .collect::<ResultV1<Vec<_>>>()?;
        require(
            entries.iter().all(|f| {
                f.role == FunctionRole::KernelEntry
                    && f.body.is_some()
                    && f.signature == entries[0].signature
            }) && entries[0].signature.results.is_empty()
                && kernel.workgroup_size == Some(WorkgroupSize::new(64, 1, 1))
                && matches!(
                    kernel.domain,
                    LaunchDomain::D1 {
                        x: LaunchExtent::Dynamic
                    }
                ),
            Phase::Abi,
            "physical ABI/launch changed",
        )?;
        let params = &entries[0].signature.parameters;
        if case == ScalarCase::NormalNoop {
            require(params.is_empty(), Phase::Abi, "noop ABI")?;
        } else {
            require(
                params.len() == if root.name == case.roots()[0] { 3 } else { 2 },
                Phase::Abi,
                "positive ABI arity",
            )?;
            require(
                matches!(&params[0], Type::Slice(s) if s.address_space == AddressSpace::Global
                && s.access == AccessMode::ReadWrite && *s.element == Type::Scalar(ScalarType::U32))
                    && params[1..]
                        .iter()
                        .all(|p| *p == Type::Scalar(ScalarType::U32)),
                Phase::Abi,
                "exact output/U32 source ABI",
            )?;
        }
        let before = counts(entries[1])?;
        let after = counts(entries[2])?;
        check_counts(case, &root.name, before, after)?;
        match case {
            ScalarCase::NormalNoop => require(
                operations(entries[1]).all(|op| {
                    !matches!(
                        op.kind,
                        Kind::Binary { .. } | Kind::Load { .. } | Kind::Call { .. }
                    )
                }),
                Phase::Bound,
                "normal source is not a genuine no-op",
            )?,
            ScalarCase::CheckedOpt0 if root.name == case.roots()[0] => {
                require(
                    entries[1]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .filter(|b| {
                            matches!(
                                b.terminator,
                                Some(
                                    Terminator::ConditionalBranch { .. }
                                        | Terminator::IntegerSwitch { .. }
                                )
                            )
                        })
                        .count()
                        >= 2,
                    Phase::Bound,
                    "bounds/choose control missing",
                )?;
            }
            ScalarCase::CheckedOpt0 => {
                for entry in &entries[1..] {
                    require(
                        operations(entry).any(|op| {
                            matches!(&op.kind,
                        Kind::Binary { op: BinaryOp::Checked(Checked::Add), lhs, rhs }
                        if literal(entry, *rhs) == Some(1) && literal(entry, *lhs).is_none())
                        }),
                        Phase::Bound,
                        "nonneutral actual Add(+1) missing",
                    )?;
                }
            }
        }
        rows.push(RootRow {
            name: root.name.clone(),
            source_function: root.function,
            source_body: root.body,
            entry: kernel.entry.as_str().into(),
            abi: format!("{:?}", entries[0].signature),
            launch: format!("{kernel:?}"),
            before,
            after,
        });
    }
    Ok(rows)
}
fn context_stamp(stage: &Stage) -> Vec<[u8; 32]> {
    let checked = stage.checked_output();
    vec![
        digest(stage.semantic().canonical_encoding()),
        digest(stage.original_canonical_bytes()),
        stage.erased_digest().copied().unwrap_or([0; 32]),
        digest(stage.test_bound_owner_v1().canonical().canonical_bytes()),
        digest(checked.native_input_audit_bytes()),
        digest(stage.output().canonical().canonical_bytes()),
        digest(checked.execution().canonical_bytes()),
        digest(checked.intermediate_policy5().execution().canonical_bytes()),
        digest(checked.continuation().execution().canonical_bytes()),
    ]
}

fn paid<T>(
    owner: fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
    budget: &mut Budget<'_>,
    body: impl FnOnce(&fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1, &mut Budget<'_>) -> ResultV1<T>,
) -> ResultV1<T> {
    let receipt = owner.retained_storage();
    budget
        .reserve_storage(receipt)
        .map_err(|e| failure(Phase::Scalar, e))?;
    let result = body(&owner, budget);
    drop(owner);
    budget
        .release_storage(receipt)
        .map_err(|e| failure(Phase::Scalar, e))?;
    result
}
fn observe(tcx: TyCtxt<'_>, request: &ScalarRequestV1) -> ResultV1<Observation> {
    let cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    require(
        cpu == request.target.cpu(),
        Phase::Rustc,
        "actual session target changed",
    )?;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| failure(Phase::Collect, e))?;
    let ranked = transaction
        .verify_general_kernel_checks()
        .map_err(|e| failure(Phase::Ranked, format!("{e:?}")))?;
    require(
        ranked.all_kernel_checks_are_clean() && !ranked.grants_artifact_or_launch_authority(),
        Phase::Ranked,
        "source checks/authority",
    )?;
    let stage = ranked
        .lower_fixed_checked_output_policy6_v1()
        .map_err(|e| failure(Phase::Bound, format!("{e:?}")))?;
    let stamp = context_stamp(&stage);
    let floor = stage.retained_storage_floor_v1();
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    budget
        .reserve_storage(floor)
        .map_err(|e| failure(Phase::Scalar, e))?;
    let result = (|| {
        let input = stage.test_bound_owner_v1();
        let scalar = prepare(input, &mut budget).map_err(|e| failure(Phase::Scalar, e))?;
        require(
            budget.storage() == floor,
            Phase::Scalar,
            "factory failed to restore prepaid context floor",
        )?;
        paid(scalar, &mut budget, |scalar, budget| {
            let start = budget.work();
            scalar
                .replay_against(input, budget)
                .map_err(|e| failure(Phase::Replay, e))?;
            let replay_work = budget.work() - start;
            let mut previous = input;
            let mut rounds = Vec::new();
            for (ordinal, round) in scalar.rounds().iter().enumerate() {
                require(
                    round.ordinal() as usize == ordinal,
                    Phase::Replay,
                    "round ordinal",
                )?;
                rounds.push(RoundRow {
                    ordinal: round.ordinal(),
                    input: subject(previous),
                    integer: subject(round.integer().owner()),
                    output: subject(round.output()),
                    integer_execution: digest(round.integer().execution().canonical_bytes()),
                    scalar_execution: digest(round.scalar().execution().canonical_bytes()),
                    changed: previous.canonical().canonical_bytes()
                        != round.output().canonical().canonical_bytes(),
                });
                previous = round.output();
            }
            require(
                !rounds.is_empty()
                    && !rounds.last().unwrap().changed
                    && rounds[..rounds.len() - 1].iter().all(|r| r.changed)
                    && !scalar.grants_authority()
                    && !scalar.authenticates_compiler_origin()
                    && !scalar.execution().grants_authority(),
                Phase::Replay,
                "complete terminal round/no authority",
            )?;
            require(
                match request.case {
                    ScalarCase::CheckedOpt0 => {
                        input.canonical().canonical_bytes()
                            != scalar.output().canonical().canonical_bytes()
                            && rounds.iter().any(|r| r.changed)
                    }
                    ScalarCase::NormalNoop => {
                        input.canonical().canonical_bytes()
                            == scalar.output().canonical().canonical_bytes()
                            && rounds.len() == 1
                    }
                },
                Phase::Scalar,
                "source positive/no-op contract",
            )?;
            component_target::binding(
                stage.test_prebind_owner_v1(),
                input,
                request.target,
                budget,
            )?;
            let rows = roots(&stage, scalar.output(), request.case)?;
            let second_floor = budget.storage();
            let second = prepare(scalar.output(), budget).map_err(|e| failure(Phase::Scalar, e))?;
            require(
                budget.storage() == second_floor,
                Phase::Scalar,
                "second factory floor",
            )?;
            let idempotent_storage = second.retained_storage();
            paid(second, budget, |second, budget| {
                second
                    .replay_against(scalar.output(), budget)
                    .map_err(|e| failure(Phase::Replay, e))?;
                require(
                    second.rounds().len() == 1
                        && second.output().canonical().canonical_bytes()
                            == scalar.output().canonical().canonical_bytes(),
                    Phase::Scalar,
                    "independent second full schedule not fixed",
                )
            })?;
            let simulations = simulate_matrix(input, scalar.output(), request.case)?;
            require(
                stamp == context_stamp(&stage),
                Phase::Replay,
                "retained source/N/E/B/Policy6 context mutated",
            )?;
            Ok(Observation {
                actual_target: request.target,
                semantic: Subject {
                    digest: *stage.semantic().semantic_sha256().as_bytes(),
                    bytes: stage.semantic().canonical_encoding().len(),
                },
                original: subject(stage.test_original_owner_v1()),
                erased: stage.erased_digest().copied(),
                bound: subject(input),
                output: subject(scalar.output()),
                roots: rows,
                terminal_round: rounds.len() - 1,
                rounds,
                execution: digest(scalar.execution().canonical_bytes()),
                replay_work,
                retained_floor: floor,
                history_storage: scalar.retained_storage(),
                idempotent_storage,
                peak_storage: budget.peak_storage(),
                component_work: budget.work(),
                final_storage: usize::MAX,
                simulations,
            })
        })
    })();
    require(
        budget.storage() == floor,
        Phase::Scalar,
        "component cleanup did not restore stage floor",
    )?;
    drop(stage);
    budget
        .release_storage(floor)
        .map_err(|e| failure(Phase::Scalar, e))?;
    result.map(|mut result| {
        result.final_storage = budget.storage();
        result
    })
}

fn validate(request: &ScalarRequestV1, row: &Observation) -> ResultV1<()> {
    require(
        row.actual_target == request.target
            && row.semantic.digest != [0; 32]
            && [&row.semantic, &row.original, &row.bound, &row.output]
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
        row.roots.len() == request.case.roots().len()
            && row
                .roots
                .iter()
                .map(|r| r.name.as_str())
                .collect::<BTreeSet<_>>()
                == request.case.roots().iter().copied().collect()
            && row.roots.iter().all(|r| {
                r.source_function != [0; 32]
                    && r.source_body != [0; 32]
                    && !r.entry.is_empty()
                    && !r.abi.is_empty()
                    && !r.launch.is_empty()
            }),
        Phase::Abi,
        "observation actual source/ABI roster",
    )?;
    require(
        !row.rounds.is_empty()
            && row.rounds.len() <= fe2o3_kernel_opt::SCALAR_FIXED_POINT_MAX_ROUNDS_V1
            && row.terminal_round == row.rounds.len() - 1,
        Phase::Replay,
        "observation terminal extent",
    )?;
    let mut input = &row.bound;
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
                ScalarCase::CheckedOpt0 => row.bound != row.output && row.rounds.len() >= 2,
                ScalarCase::NormalNoop => row.bound == row.output && row.rounds.len() == 1,
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
            schema: 1,
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
            "SCALAR_B_COMPONENT {} {case:?} callbacks=1 roots={} scenarios={} sim_executions={}",
            target.cpu(),
            observed.roots.len(),
            observed.simulations.len(),
            observed.simulations.len() * 4
        );
    }
}
#[test]
#[ignore = "actual Cargo/rustc B-to-scalar component qualification on both profiles"]
fn ordinary_rust_scalar_fixed_point_b_component_checked_opt0_both_profiles() {
    qualify(ScalarCase::CheckedOpt0);
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
        schema: 1,
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
        "actual_target": request.target, "semantic": subject, "original": subject,
        "erased": null, "bound": subject, "output": subject,
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
    assert_eq!(scenarios(ScalarCase::CheckedOpt0).len(), 60);
    assert_eq!(scenarios(ScalarCase::NormalNoop).len(), 1);
    assert_eq!((60 + 1) * 2 * 4, 488);
    for case in [ScalarCase::CheckedOpt0, ScalarCase::NormalNoop] {
        let args = executed(&["rustc".into(), "-Coverflow-checks=on".into()], case).unwrap();
        assert_eq!(args.len(), 4);
        assert_eq!(args[2], format!("--check-cfg=cfg({CFG})"));
        assert_eq!(
            args[3],
            if case == ScalarCase::CheckedOpt0 {
                "-Zmir-opt-level=0".into()
            } else {
                format!("--cfg={CFG}")
            }
        );
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
            0 => request.case = ScalarCase::CheckedOpt0,
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
