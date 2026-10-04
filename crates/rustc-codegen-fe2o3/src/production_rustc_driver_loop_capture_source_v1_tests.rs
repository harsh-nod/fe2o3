//! Strict ordinary-source capture/query only. No final output or optimizer claim.
use super::*;
use crate::production_pipeline::ProductionPipelineError;
use crate::production_ranked_projection_v1::scalar_emission_capture_v1::{
    CapturedBoundSnapshotSourceV1, SourceBoundSnapshotObservationV1 as SourceLoopObservationV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ScalarType,
};
use fe2o3_lower_mir_kernel::ProductionU32BoundSnapshotRecurrenceV1 as Consistency;
use fe2o3_mir_model::semantic_mir_v1::{SemanticScalarTypeV1, SemanticTypeShapeV1};

#[path = "production_rustc_driver_loop_guard_source_v1_tests.rs"]
mod guard;

#[path = "production_rustc_driver_loop_guard_protocol_closure_v1_tests.rs"]
mod protocol_closure;

#[path = "production_rustc_driver_guarded_loop_source_v1_tests.rs"]
mod production_guarded;

const REQUEST: &str = "FE2O3_TEST_LOOP_CAPTURE_REQUEST_V1";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
enum Case {
    Exact,
    Renamed,
    Generic,
    Multi,
    U64,
}
impl Case {
    fn feature(self) -> &'static str {
        match self {
            Self::Exact => "loop-capture-exact",
            Self::Renamed => "loop-capture-renamed",
            Self::Generic => "loop-capture-generic",
            Self::Multi => "loop-capture-multi",
            Self::U64 => "loop-capture-u64",
        }
    }
    fn roots(self) -> usize {
        if self == Self::Multi { 2 } else { 1 }
    }
    fn names(self) -> &'static [&'static str] {
        match self {
            Self::Exact => &["count_to_limit"],
            Self::Renamed => &["renamed_count"],
            Self::Generic => &["generic_count"],
            Self::Multi => &["count_to_limit", "renamed_count"],
            Self::U64 => &["wide_copy"],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Stamp {
    path: PathBuf,
    sha256: [u8; 32],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Request {
    case: Case,
    target: String,
    args_sha256: [u8; 32],
    source: Vec<Stamp>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
enum Stage {
    Request,
    Rustc,
    SourceCollection,
    Capture,
    Ranked,
    Query,
    Guard,
    Observation,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    stage: Stage,
    detail: String,
}
fn fail(stage: Stage, detail: impl std::fmt::Display) -> Failure {
    Failure {
        stage,
        detail: detail.to_string(),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::large_enum_variant,
    reason = "The test protocol keeps each complete observation in one serialized row"
)]
enum Outcome {
    NoCertificate,
    Unavailable(String),
    Joined {
        header: [u32; 2],
        parameter: String,
        initial: String,
        update: String,
        step: String,
        overflow: String,
        initial_edge: String,
        backedge: String,
        source_initialization: String,
        source_update: String,
        source_guard: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    root: u32,
    body: u32,
    root_identity: [u8; 32],
    body_identity: [u8; 32],
    report_semantic: [u8; 32],
    ordinal: Option<usize>,
    certificates: usize,
    checked_additions: usize,
    dynamic_u32_bound: bool,
    outcome: Outcome,
    guard: guard::Outcome,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    semantic: [u8; 32],
    n: [u8; 32],
    roots: Vec<u32>,
    rows: Vec<Row>,
    query_work: usize,
    guard_work: usize,
    retained_floor: usize,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    result: Result<Observation, Failure>,
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn source_paths(workspace: &Path) -> [PathBuf; 4] {
    [
        workspace.join("Cargo.lock"),
        workspace.join(format!("{BASE}/Cargo.toml")),
        workspace.join(format!("{BASE}/src/lib.rs")),
        workspace.join(format!("{BASE}/src/loop_capture.rs")),
    ]
}
fn stamps(workspace: &Path) -> Vec<Stamp> {
    source_paths(workspace)
        .into_iter()
        .map(|path| Stamp {
            sha256: digest(&std::fs::read(&path).unwrap()),
            path,
        })
        .collect()
}
fn check_request(request: &Request, args: &[String]) -> Result<(), Failure> {
    if !["gfx942", "gfx950"].contains(&request.target.as_str())
        || request.args_sha256 != digest(&serde_json::to_vec(args).unwrap())
        || request.source.len() != 4
    {
        return Err(fail(Stage::Request, "exact target, argv or source roster"));
    }
    require_canonical_overflow_checks_v1(args).map_err(|e| fail(Stage::Request, e))?;
    let expected_cfg = format!("feature=\"{}\"", request.case.feature());
    if args
        .windows(2)
        .filter(|pair| pair[0] == "--cfg" && pair[1] == expected_cfg)
        .count()
        != 1
        || args
            .iter()
            .any(|arg| arg.starts_with("feature=\"loop-capture-") && arg != &expected_cfg)
        || args
            .iter()
            .any(|arg| arg.starts_with("--cfg=feature=\"loop-capture-"))
        || args
            .iter()
            .filter(|arg| arg.as_str() == "-Zmir-opt-level=0")
            .count()
            != 1
        || args.iter().any(|arg| {
            (arg.starts_with("-Zmir-opt-level") && arg != "-Zmir-opt-level=0")
                || arg.starts_with("mir-opt-level")
        })
    {
        return Err(fail(
            Stage::Request,
            "actual fixture cfg or explicit opt0 missing",
        ));
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| fail(Stage::Request, e))?;
    if request
        .source
        .iter()
        .map(|stamp| &stamp.path)
        .ne(source_paths(&workspace).iter())
    {
        return Err(fail(
            Stage::Request,
            "source roster is not the exact current fixture closure",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for stamp in &request.source {
        if !seen.insert(&stamp.path)
            || digest(&std::fs::read(&stamp.path).map_err(|e| fail(Stage::Request, e))?)
                != stamp.sha256
        {
            return Err(fail(
                Stage::Request,
                "duplicate, absent or changed source stamp",
            ));
        }
    }
    Ok(())
}

fn observe(tcx: TyCtxt<'_>, target: &str, diagnostic: &mut String) -> Result<Observation, Failure> {
    let active_cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    if active_cpu != target {
        return Err(fail(
            Stage::Request,
            "actual rustc target does not match request",
        ));
    }
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| fail(Stage::SourceCollection, e))?;
    let stage = transaction
        .capture_ranked_scalar_emission_v1()
        .map_err(|error| {
            let kind = match error.as_ref() {
                ProductionPipelineError::ScalarEmissionCapture(_) => Stage::Capture,
                ProductionPipelineError::RankedProjection(_) => Stage::Ranked,
                _ => Stage::SourceCollection,
            };
            fail(kind, format!("{error:?}"))
        })?;
    if stage.grants_authority() {
        return Err(fail(Stage::Observation, "capture acquired authority"));
    }
    let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
        .map_err(|e| fail(Stage::Query, e))?;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    budget
        .reserve_storage(stage.retained_storage())
        .map_err(|e| fail(Stage::Query, e))?;
    let stage = CapturedBoundSnapshotSourceV1::try_attach_v1(stage, &mut budget)
        .map_err(|e| fail(Stage::Query, e))?;
    if stage.grants_authority() {
        return Err(fail(
            Stage::Observation,
            "snapshot attachment acquired authority",
        ));
    }
    let original = stage.capture().original();
    let source = original.semantic_ssa().source_semantic();
    let source_identity = *source.semantic_sha256().as_bytes();
    let n_identity = *original.executable().canonical().identity().digest();
    let count = stage
        .observation_count_v1(&mut budget)
        .map_err(|e| fail(Stage::Query, e))?;
    let mut slots: Vec<Option<SourceLoopObservationV1<'_>>> = Vec::new();
    let requested = count
        .checked_mul(std::mem::size_of::<Option<SourceLoopObservationV1<'_>>>())
        .ok_or_else(|| fail(Stage::Observation, "diagnostic staging overflow"))?;
    budget
        .reserve_storage(requested)
        .map_err(|e| fail(Stage::Query, e))?;
    slots
        .try_reserve_exact(count)
        .map_err(|e| fail(Stage::Observation, e))?;
    let bytes = slots
        .capacity()
        .checked_mul(std::mem::size_of::<Option<SourceLoopObservationV1<'_>>>())
        .ok_or_else(|| fail(Stage::Observation, "diagnostic capacity overflow"))?;
    budget
        .reserve_storage(
            bytes
                .checked_sub(requested)
                .ok_or_else(|| fail(Stage::Observation, "capacity underflow"))?,
        )
        .map_err(|e| fail(Stage::Query, e))?;
    slots.resize_with(count, || None);
    let floor = budget.storage();
    let before = budget.work();
    let mut next = 0;
    stage
        .with_observations_v1(&mut budget, |row, _| {
            let slot = slots.get_mut(next).ok_or(
                fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionErrorV1::Mismatch(
                    "extra observation",
                ),
            )?;
            *slot = Some(row);
            next += 1;
            Ok(())
        })
        .map_err(|e| fail(Stage::Query, e))?;
    if next != count || budget.storage() != floor {
        return Err(fail(Stage::Observation, "incomplete observations or floor"));
    }
    let query_work = budget.work() - before;
    let before_guard = budget.work();
    let ledger = budget.work_ledger_identity_v1();
    let (guards, guard_storage) = guard::analyze(&stage, &slots, &mut budget)?;
    if budget.storage() != floor
        || !std::ptr::eq(guards.owner(), stage.capture())
        || guard_storage != guards.storage()
        || guards.authorizes_compiler_transform()
    {
        return Err(fail(Stage::Guard, "guard report owner, floor or authority"));
    }
    budget
        .reserve_storage(guard_storage.retained_storage())
        .map_err(|e| fail(Stage::Guard, e))?;
    let guard_floor = budget.storage();
    let mut next_guard = 0;
    // Serialization and strings below are test diagnostics, outside the query scope.
    let mut rows = Vec::new();
    for slot in &slots {
        let observed = slot
            .as_ref()
            .ok_or_else(|| fail(Stage::Observation, "missing observation"))?;
        let root = observed.root().semantic_root();
        let selection = source
            .select_kernel_body_for_root_v1(root)
            .ok_or_else(|| fail(Stage::Observation, "actual body selection"))?;
        let body = &source.functions()[selection.body().index() as usize];
        let report = observed.report();
        if report.function() != selection.body()
            || report.function_identity() != body.identity()
            || report.semantic_mir_sha256() != source.semantic_sha256()
        {
            return Err(fail(
                Stage::Observation,
                "bound-snapshot report source custody",
            ));
        }
        let mut dynamic_u32_bound = false;
        let outcome = match observed.outcome() {
            None => Outcome::NoCertificate,
            Some(Consistency::Unavailable(reason)) => Outcome::Unavailable(format!("{reason:?}")),
            Some(Consistency::Joined(fact)) => {
                if !std::ptr::eq(fact.source(), original)
                    || fact.root() != root
                    || fact.authorizes_compiler_transform()
                {
                    return Err(fail(Stage::Observation, "actual source fact custody"));
                }
                let certificate = fact.certificate();
                if Some(&certificate)
                    != observed
                        .certificate_ordinal()
                        .and_then(|i| report.certificates().get(i))
                {
                    return Err(fail(Stage::Observation, "certificate ordinal changed"));
                }
                let bound = &body.locals()[certificate.bound().local().index() as usize];
                dynamic_u32_bound = bound.role().is_entry_argument()
                    && matches!(
                        source.types()[bound.ty().index() as usize].shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 32
                        })
                    );
                let recurrence = fact.recurrence();
                if recurrence.scalar() != ScalarType::U32
                    || recurrence.step_bits() != 1
                    || recurrence.overflow().is_none()
                {
                    return Err(fail(Stage::Observation, "exact U32 recurrence"));
                }
                let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument {
                    block,
                    ..
                } = recurrence.parameter()
                else {
                    return Err(fail(Stage::Observation, "header parameter"));
                };
                Outcome::Joined {
                    header: [block.function.0, block.block],
                    parameter: format!("{:?}", recurrence.parameter()),
                    initial: format!("{:?}", recurrence.initial()),
                    update: format!("{:?}", recurrence.update()),
                    step: format!("{:?}", recurrence.step()),
                    overflow: format!("{:?}", recurrence.overflow()),
                    initial_edge: format!("{:?}", recurrence.initial_edge()),
                    backedge: format!("{:?}", recurrence.backedge()),
                    source_initialization: format!("{:?}", certificate.initialization()),
                    source_update: format!("{:?}", certificate.update()),
                    source_guard: format!("{:?}", certificate.guard()),
                }
            }
        };
        source_diagnostic::capture(
            diagnostic,
            original,
            root,
            report,
            observed.certificate_ordinal(),
            &outcome,
        );
        rows.push(Row {
            root: root.index(),
            body: selection.body().index(),
            root_identity: *source.functions()[root.index() as usize]
                .identity()
                .as_bytes(),
            body_identity: *body.identity().as_bytes(),
            report_semantic: *report.semantic_mir_sha256().as_bytes(),
            ordinal: observed.certificate_ordinal(),
            certificates: report.certificates().len(),
            checked_additions: report.checked_additions_examined(),
            dynamic_u32_bound,
            outcome,
            guard: guard::observe(
                &stage,
                observed,
                guards.rows(),
                &mut next_guard,
                &mut budget,
            )?,
        });
    }
    if next_guard != guards.rows().len()
        || budget.storage() != guard_floor
        || budget.work_ledger_identity_v1() != ledger
    {
        return Err(fail(
            Stage::Guard,
            "complete guard requests, ledger or retained receipt",
        ));
    }
    let guard_work = budget.work() - before_guard;
    drop(guards);
    budget
        .release_storage(guard_storage.retained_storage())
        .map_err(|e| fail(Stage::Guard, e))?;
    drop(slots);
    budget
        .release_storage(bytes)
        .map_err(|e| fail(Stage::Query, e))?;
    if budget.storage() != stage.retained_storage() {
        return Err(fail(
            Stage::Observation,
            "query did not restore retained floor",
        ));
    }
    Ok(Observation {
        semantic: source_identity,
        n: n_identity,
        roots: source.roots().iter().map(|root| root.index()).collect(),
        rows,
        query_work,
        guard_work,
        retained_floor: stage.retained_storage(),
    })
}

struct LoopCallbacks {
    target: String,
    result: Option<Result<Observation, Failure>>,
    diagnostic: String,
}
impl Callbacks for LoopCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(observe(tcx, &self.target, &mut self.diagnostic));
        Compilation::Stop
    }
}
pub(super) fn requested() -> bool {
    env::var_os(REQUEST).is_some() || production_guarded::requested()
}
pub(super) fn run_child(args: &[String]) {
    if production_guarded::requested() {
        production_guarded::run_child(args);
        return;
    }
    let request: Request = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    let path = PathBuf::from(env::var_os(CHILD_RESULT).unwrap());
    assert!(
        !path.exists(),
        "source capture requires a fresh response path"
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request(&request, args)?;
        let mut callbacks = LoopCallbacks {
            target: request.target.clone(),
            result: None,
            diagnostic: String::new(),
        };
        rustc_driver::run_compiler(args, &mut callbacks);
        // Print last, after rustc warning summaries, so the parent's bounded tail retains it.
        if !callbacks.diagnostic.is_empty() {
            eprint!("{}", callbacks.diagnostic);
        }
        check_request(&request, args)?;
        callbacks
            .result
            .ok_or_else(|| fail(Stage::Rustc, "actual loop callback absent"))?
    }))
    .unwrap_or_else(|_| Err(fail(Stage::Rustc, "compiler or capture callback panicked")));
    let success = result.is_ok();
    let report = Report { request, result };
    std::fs::write(path, serde_json::to_vec(&report).unwrap()).unwrap();
    assert!(success, "strict source/N capture: {report:?}");
}

fn validate(request: &Request, observed: &Observation) -> Result<(), String> {
    let unique: std::collections::BTreeSet<_> = observed.roots.iter().copied().collect();
    if observed.semantic == [0; 32]
        || observed.n == [0; 32]
        || observed.query_work == 0
        || observed.guard_work == 0
        || observed.retained_floor == 0
        || observed.roots.len() != request.case.roots()
        || unique.len() != observed.roots.len()
        || observed.rows.len() != observed.roots.len()
    {
        return Err("missing source/N identity, query or exact unique root roster".into());
    }
    for (root, row) in observed.roots.iter().zip(&observed.rows) {
        if root != &row.root
            || row.report_semantic != observed.semantic
            || row.root_identity == [0; 32]
            || row.body_identity == [0; 32]
        {
            return Err("row is not bound to the actual root/report order".into());
        }
        guard::validate(row)?;
        if request.case == Case::U64 {
            if row.ordinal.is_some()
                || row.certificates != 0
                || row.outcome != Outcome::NoCertificate
            {
                return Err("unsupported U64 acquired a U32 certificate/join".into());
            }
        } else if row.ordinal != Some(0)
            || row.certificates != 1
            || row.checked_additions == 0
            || !row.dynamic_u32_bound
            || !matches!(row.outcome, Outcome::Joined { .. })
        {
            return Err(format!(
                "strict positive requires a genuine dynamic U32 source/N join: {row:?}"
            ));
        }
    }
    Ok(())
}
fn decode(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    request: &Request,
) -> Result<Observation, String> {
    if status != Some(0) {
        return Err(format!("strict child status {status:?}"));
    }
    let report: Report =
        serde_json::from_slice(bytes.ok_or("missing fresh report")?).map_err(|e| e.to_string())?;
    if report.request != *request {
        return Err("foreign report request".into());
    }
    let observed = report
        .result
        .map_err(|e| format!("{:?}: {}", e.stage, e.detail))?;
    validate(request, &observed)?;
    Ok(observed)
}

fn fixture(workspace: &Path, case: Case, target: &str) -> corpus::Fixture {
    let hash = |path: &str| {
        digest(&std::fs::read(workspace.join(path)).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    let manifest = format!("{BASE}/Cargo.toml");
    corpus::Fixture {
        fixture_id: format!("{}-{target}", case.feature()),
        target: target.into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/loop_capture.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.feature().into()],
            kernel_symbols: case.names().iter().map(|name| (*name).into()).collect(),
        },
    }
}

#[test]
#[ignore = "strict actual dynamic device loops through source/SSA/N capture; pinned nightly and AMD dependencies"]
fn ordinary_rust_dynamic_u32_loops_capture_and_join_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-loop-capture-source");
    let source = stamps(&workspace);
    let mut completed = 0;
    let mut joined = 0;
    let mut roots = 0;
    let mut guard_joined = 0;
    let mut no_certificate = 0;
    for target in ["gfx942", "gfx950"] {
        for case in [
            Case::Exact,
            Case::Renamed,
            Case::Generic,
            Case::Multi,
            Case::U64,
        ] {
            let directory = scratch.path().join(format!("{}-{target}", case.feature()));
            std::fs::create_dir(&directory).unwrap();
            let mut captured = corpus_cargo::capture(
                &workspace,
                &fixture(&workspace, case, target),
                &directory,
                &scratch.path().join(target),
            )
            .unwrap();
            require_canonical_overflow_checks_v1(&captured.args).unwrap();
            captured.args.push("-Zmir-opt-level=0".into());
            let request = Request {
                case,
                target: target.into(),
                args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
                source: source.clone(),
            };
            let args = directory.join("args.json");
            let report = directory.join("result.json");
            std::fs::write(&args, serde_json::to_vec(&captured.args).unwrap()).unwrap();
            assert!(!report.exists());
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
                .env(REQUEST, serde_json::to_string(&request).unwrap())
                .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"]);
            progress::clear_inherited_jobserver(&mut command);
            fixed_census_observation::configure(&mut command, None);
            let output = command.output().unwrap();
            let bytes = std::fs::read(&report);
            let observed = decode(output.status.code(), bytes.as_deref().ok(), &request)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} {target}: {error}\n{}",
                        case.feature(),
                        corpus_cargo::diagnostics(&output)
                    )
                });
            joined += observed
                .rows
                .iter()
                .filter(|row| matches!(row.outcome, Outcome::Joined { .. }))
                .count();
            roots += observed.roots.len();
            guard_joined += observed
                .rows
                .iter()
                .filter(|row| matches!(row.guard, guard::Outcome::Joined { .. }))
                .count();
            no_certificate += observed
                .rows
                .iter()
                .filter(|row| row.guard == guard::Outcome::NoCertificate)
                .count();
            completed += 1;
            assert_eq!(stamps(&workspace), source);
            eprintln!(
                "LOOP CAPTURE {} {target}: roots={}, joined={}, guard_joined={}, N={:02x?}; inert original-N queries only",
                case.feature(),
                observed.roots.len(),
                observed
                    .rows
                    .iter()
                    .filter(|row| matches!(row.outcome, Outcome::Joined { .. }))
                    .count(),
                observed
                    .rows
                    .iter()
                    .filter(|row| matches!(row.guard, guard::Outcome::Joined { .. }))
                    .count(),
                observed.n
            );
        }
    }
    assert_eq!((completed, joined), (10, 10));
    assert_eq!((roots, guard_joined, no_certificate), (12, 10, 2));
}

#[test]
fn strict_loop_observer_distinguishes_empty_unavailable_refusal_and_foreign_reports() {
    // Synthetic protocol controls are not source-capture or recurrence evidence.
    let request = Request {
        case: Case::Exact,
        target: "gfx942".into(),
        args_sha256: [1; 32],
        source: vec![],
    };
    let joined = Outcome::Joined {
        header: [0, 1],
        parameter: "p".into(),
        initial: "i".into(),
        update: "u".into(),
        step: "s".into(),
        overflow: "o".into(),
        initial_edge: "e".into(),
        backedge: "b".into(),
        source_initialization: "i".into(),
        source_update: "u".into(),
        source_guard: "g".into(),
    };
    let observed = Observation {
        semantic: [2; 32],
        n: [3; 32],
        roots: vec![7],
        query_work: 1,
        guard_work: 1,
        retained_floor: 2,
        rows: vec![Row {
            root: 7,
            body: 9,
            root_identity: [4; 32],
            body_identity: [5; 32],
            report_semantic: [2; 32],
            ordinal: Some(0),
            certificates: 1,
            checked_additions: 1,
            dynamic_u32_bound: true,
            outcome: joined,
            guard: guard::Outcome::Joined {
                header: [0, 1],
                bound: [0, 0],
                condition: [0, 1, 0, 0],
                body: [0, 2],
                exit: [0, 3],
                then_edge: [0, 1, 0],
                else_edge: [0, 1, 1],
            },
        }],
    };
    let bytes = serde_json::to_vec(&Report {
        request: request.clone(),
        result: Ok(observed.clone()),
    })
    .unwrap();
    decode(Some(0), Some(&bytes), &request).unwrap();
    for status in [None, Some(1), Some(101)] {
        assert!(decode(status, Some(&bytes), &request).is_err());
    }
    for data in [None, Some(b"{}".as_slice()), Some(b"not-json".as_slice())] {
        assert!(decode(Some(0), data, &request).is_err());
    }
    for outcome in [
        Outcome::NoCertificate,
        Outcome::Unavailable("NoExactRecurrence".into()),
    ] {
        let mut changed = observed.clone();
        changed.rows[0].outcome = outcome;
        assert!(validate(&request, &changed).is_err());
    }
    for modify in 0..5 {
        let mut changed = observed.clone();
        match modify {
            0 => changed.rows[0].root = 8,
            1 => changed.rows[0].report_semantic = [9; 32],
            2 => changed.rows[0].dynamic_u32_bound = false,
            3 => changed.roots.push(7),
            _ => changed.guard_work = 0,
        }
        assert!(validate(&request, &changed).is_err());
    }
    let mut foreign = request.clone();
    foreign.args_sha256 = [8; 32];
    assert!(decode(Some(0), Some(&bytes), &foreign).is_err());
    let refusal = serde_json::to_vec(&Report {
        request: request.clone(),
        result: Err(fail(Stage::Query, "refused")),
    })
    .unwrap();
    assert!(decode(Some(0), Some(&refusal), &request).is_err());
    let mut unsupported = observed;
    unsupported.rows[0].outcome = Outcome::NoCertificate;
    unsupported.rows[0].guard = guard::Outcome::NoCertificate;
    unsupported.rows[0].certificates = 0;
    unsupported.rows[0].ordinal = None;
    let mut request = request;
    request.case = Case::U64;
    validate(&request, &unsupported).unwrap();
}

#[test]
fn loop_request_rejects_source_substitution_and_conflicting_capture_options() {
    // Request validation only; this does not invoke rustc or synthesize a fact.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let args = vec![
        "rustc".into(),
        "--cfg".into(),
        "feature=\"loop-capture-exact\"".into(),
        "-Coverflow-checks=on".into(),
        "-Zmir-opt-level=0".into(),
    ];
    let request = Request {
        case: Case::Exact,
        target: "gfx942".into(),
        args_sha256: digest(&serde_json::to_vec(&args).unwrap()),
        source: stamps(&workspace),
    };
    check_request(&request, &args).unwrap();
    for mode in 0..5 {
        let mut changed = request.clone();
        match mode {
            0 => changed.source.swap(0, 1),
            1 => changed.source[0] = changed.source[1].clone(),
            2 => changed.source[0].path = workspace.join("Cargo.toml"),
            3 => changed.source[0].sha256 = [0; 32],
            _ => changed.target = "gfx999".into(),
        }
        assert!(matches!(
            check_request(&changed, &args),
            Err(Failure {
                stage: Stage::Request,
                ..
            })
        ));
    }
    for extra in [
        vec!["-Zmir-opt-level=0"],
        vec!["-Zmir-opt-level=2"],
        vec!["-Z", "mir-opt-level=2"],
        vec!["--cfg", "feature=\"loop-capture-renamed\""],
        vec!["--cfg=feature=\"loop-capture-renamed\""],
        vec!["-Coverflow-checks=off"],
    ] {
        let mut changed_args = args.clone();
        changed_args.extend(extra.into_iter().map(str::to_owned));
        let mut changed = request.clone();
        changed.args_sha256 = digest(&serde_json::to_vec(&changed_args).unwrap());
        assert!(matches!(
            check_request(&changed, &changed_args),
            Err(Failure {
                stage: Stage::Request,
                ..
            })
        ));
    }
    let mut changed = request;
    changed.args_sha256 = [0; 32];
    assert!(matches!(
        check_request(&changed, &args),
        Err(Failure {
            stage: Stage::Request,
            ..
        })
    ));
}

#[path = "production_rustc_driver_loop_capture_diagnostics_v1_tests.rs"]
mod source_diagnostic;
