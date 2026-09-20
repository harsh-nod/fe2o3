//! Separate strict qualifier of the owning pre-projection guarded source stage.
//! No native emission, legacy evidence conversion or default admission claim.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

#[path = "production_rustc_driver_guarded_loop_protocol_v1_tests.rs"]
mod protocol_tests;

const NEW_REQUEST: &str = "FE2O3_TEST_GUARDED_LOOP_SOURCE_REQUEST_V1";
const FLOOR: usize = 29;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
enum Protocol {
    StrictGuardedOriginalN {},
}
const PROTOCOL: Protocol = Protocol::StrictGuardedOriginalN {};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Envelope {
    protocol: Protocol,
    capture: Request,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
enum Consumed {
    NoU32Credit {},
    SourceAndN {
        function: u32,
        block: u32,
        statement: u32,
        certificate: usize,
        dynamic_u32_bound: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservedRoot {
    root: u32,
    body: u32,
    root_identity: [u8; 32],
    body_identity: [u8; 32],
    report_semantic: [u8; 32],
    ranked_sha256: [u8; 32],
    ranked_len: usize,
    certificates: usize,
    checked_additions: usize,
    consumed: Consumed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observed {
    semantic: [u8; 32],
    n: [u8; 32],
    n_len: usize,
    roots: Vec<u32>,
    rows: Vec<ObservedRoot>,
    constructed_work: usize,
    replay_work: usize,
    stage_receipt: usize,
    unrelated_floor: usize,
    live_floor: usize,
    undercut_refused_before_work: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    protocol: Protocol,
    request: Envelope,
    result: Result<Observed, Failure>,
}

fn parse_envelope(text: &str, old_present: bool) -> Result<Envelope, String> {
    if old_present {
        return Err("guarded and historical requests are mutually exclusive".into());
    }
    serde_json::from_str(text).map_err(|error| error.to_string())
}
pub(super) fn requested() -> bool {
    env::var_os(NEW_REQUEST).is_some()
}

fn observe_owned(tcx: TyCtxt<'_>, target: &str) -> Result<Observed, Failure> {
    let actual_target = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    if actual_target != target {
        return Err(fail(Stage::Request, "actual target changed"));
    }
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|error| fail(Stage::SourceCollection, error))?;
    let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
        .map_err(|error| fail(Stage::Guard, error))?;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    budget
        .reserve_storage(FLOOR)
        .map_err(|error| fail(Stage::Guard, error))?;
    let ledger = budget.work_ledger_identity_v1();
    let stage = transaction
        .prepare_guarded_ranked_source_v1(&mut budget)
        .map_err(|error| {
            let stage = match error.as_ref() {
                ProductionPipelineError::ScalarEmissionCapture(_)
                | ProductionPipelineError::PreRankedMaterialization(_) => Stage::Capture,
                ProductionPipelineError::RankedProjection(_) => Stage::Ranked,
                _ => Stage::SourceCollection,
            };
            fail(stage, format!("{error:?}"))
        })?;
    let constructed_work = budget.work();
    let receipt = stage.retained_storage();
    let floor = budget.storage();
    if stage.grants_authority()
        || stage.source().grants_authority()
        || floor
            != FLOOR
                .checked_add(receipt)
                .ok_or_else(|| fail(Stage::Guard, "receipt overflow"))?
        || budget.work_ledger_identity_v1() != ledger
    {
        return Err(fail(
            Stage::Guard,
            "owning source stage, complete receipt or ledger",
        ));
    }
    stage
        .replay_consistency_v1(&mut budget)
        .map_err(|error| fail(Stage::Guard, format!("{error:?}")))?;
    let replay_work = budget.work() - constructed_work;
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err(fail(
            Stage::Guard,
            "replay changed its inherited floor/ledger",
        ));
    }
    // This real retained transaction must bind the unrelated entry reservation,
    // not merely require enough storage for its own receipt.
    budget
        .release_storage(1)
        .map_err(|error| fail(Stage::Guard, error))?;
    let before = (budget.work(), budget.storage());
    let refused = stage
        .replay_consistency_v1(&mut budget)
        .is_err_and(|error| {
            matches!(
                error.as_ref(),
                ProductionPipelineError::PreRankedMaterialization(
                    fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Canonical(
                        fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                            Resource::Accounting
                        )
                    )
                )
            )
        });
    if !refused || (budget.work(), budget.storage()) != before || budget.storage() < receipt {
        return Err(fail(
            Stage::Guard,
            "same-ledger undercut was not refused before work",
        ));
    }
    budget
        .reserve_storage(1)
        .map_err(|error| fail(Stage::Guard, error))?;
    let captured = stage.source();
    let original = captured.capture().original();
    let semantic = original.semantic_ssa().source_semantic();
    let semantic_identity = *semantic.semantic_sha256().as_bytes();
    let n = *original.executable().canonical().identity().digest();
    let n_len = original.executable().canonical().canonical_bytes().len();
    let roots = semantic
        .roots()
        .iter()
        .map(|root| root.index())
        .collect::<Vec<_>>();
    if captured.roots().len() != roots.len()
        || original.source_launch().roots().len() != roots.len()
    {
        return Err(fail(
            Stage::Observation,
            "actual source/N/ranked root roster",
        ));
    }
    // Test serialization allocations are diagnostics, not new production proof tables.
    let mut rows = Vec::new();
    let mut visited = std::collections::BTreeSet::new();
    for (index, ((root, ranked), launch)) in semantic
        .roots()
        .iter()
        .zip(captured.roots())
        .zip(original.source_launch().roots())
        .enumerate()
    {
        let selection = semantic
            .select_kernel_body_for_root_v1(*root)
            .ok_or_else(|| fail(Stage::Observation, "actual source body selection"))?;
        let body = &semantic.functions()[selection.body().index() as usize];
        let report = captured
            .snapshot_report_for_root_v1(index)
            .ok_or_else(|| fail(Stage::Observation, "retained actual report missing"))?;
        if ranked.semantic_root() != *root
            || launch.selected_root() != *root
            || ranked.semantic_root_identity()
                != semantic.functions()[root.index() as usize].identity()
            || report.function() != selection.body()
            || report.function_identity() != body.identity()
            || report.semantic_mir_sha256() != semantic.semantic_sha256()
            || report.grants_authority()
            || ranked.ranked_ir().is_empty()
            || !ranked.all_kernel_checks_are_clean()
        {
            return Err(fail(
                Stage::Observation,
                "actual source/ranked/report ownership or generic checks",
            ));
        }
        let matching = captured
            .sites()
            .iter()
            .filter(|site| site.root() == root.index())
            .collect::<Vec<_>>();
        let consumed = if report.certificates().is_empty() {
            if !matching.is_empty() {
                return Err(fail(
                    Stage::Observation,
                    "empty report acquired consumption",
                ));
            }
            Consumed::NoU32Credit {}
        } else {
            let [certificate] = report.certificates() else {
                return Err(fail(Stage::Observation, "fixture exact certificate count"));
            };
            let [site] = matching.as_slice() else {
                return Err(fail(Stage::Observation, "fixture exact consumed count"));
            };
            let producer = certificate.checked_addition();
            if site.function() != selection.body().index()
                || site.certificate_ordinal() != 0
                || site.block() != producer.block().block().index()
                || site.statement() != producer.statement()
                || !visited.insert((site.root(), site.function(), site.block(), site.statement()))
            {
                return Err(fail(
                    Stage::Observation,
                    "actual source producer/consumption bijection",
                ));
            }
            let bound = &body.locals()[certificate.bound().local().index() as usize];
            let dynamic_u32_bound = bound.role().is_entry_argument()
                && bound.ty() == certificate.bound().ty()
                && matches!(
                    semantic.types()[bound.ty().index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32
                    })
                );
            Consumed::SourceAndN {
                function: site.function(),
                block: site.block(),
                statement: site.statement(),
                certificate: site.certificate_ordinal(),
                dynamic_u32_bound,
            }
        };
        rows.push(ObservedRoot {
            root: root.index(),
            body: selection.body().index(),
            root_identity: *ranked.semantic_root_identity().as_bytes(),
            body_identity: *body.identity().as_bytes(),
            report_semantic: *report.semantic_mir_sha256().as_bytes(),
            ranked_sha256: digest(ranked.ranked_ir().as_bytes()),
            ranked_len: ranked.ranked_ir().len(),
            certificates: report.certificates().len(),
            checked_additions: report.checked_additions_examined(),
            consumed,
        });
    }
    if visited.len() != captured.sites().len()
        || budget.storage() != floor
        || budget.work_ledger_identity_v1() != ledger
    {
        return Err(fail(
            Stage::Observation,
            "complete source-site coverage or live ledger",
        ));
    }
    let observed = Observed {
        semantic: semantic_identity,
        n,
        n_len,
        roots,
        rows,
        constructed_work,
        replay_work,
        stage_receipt: receipt,
        unrelated_floor: FLOOR,
        live_floor: floor,
        undercut_refused_before_work: refused,
    };
    drop(stage);
    budget
        .release_storage(receipt)
        .map_err(|error| fail(Stage::Guard, error))?;
    if budget.storage() != FLOOR {
        return Err(fail(Stage::Guard, "source owner drop/floor"));
    }
    Ok(observed)
}

struct GuardedCallbacks {
    target: String,
    result: Option<Result<Observed, Failure>>,
}
impl Callbacks for GuardedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(observe_owned(tcx, &self.target));
        Compilation::Stop
    }
}
pub(super) fn run_child(args: &[String]) {
    // Once new-mode dispatch was selected, no malformed input can select the
    // historical child. A failed envelope is a hard child/setup failure.
    let request = parse_envelope(
        &env::var(NEW_REQUEST).unwrap(),
        env::var_os(super::REQUEST).is_some(),
    )
    .unwrap();
    let path = PathBuf::from(env::var_os(CHILD_RESULT).unwrap());
    assert!(
        !path.exists(),
        "guarded source requires a fresh report path"
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request(&request.capture, args)?;
        let mut callbacks = GuardedCallbacks {
            target: request.capture.target.clone(),
            result: None,
        };
        rustc_driver::run_compiler(args, &mut callbacks);
        check_request(&request.capture, args)?;
        callbacks
            .result
            .ok_or_else(|| fail(Stage::Rustc, "guarded callback absent"))?
    }))
    .unwrap_or_else(|_| Err(fail(Stage::Rustc, "guarded source callback panicked")));
    let success = result.is_ok();
    let response = Response {
        protocol: PROTOCOL,
        request,
        result,
    };
    std::fs::write(path, serde_json::to_vec(&response).unwrap()).unwrap();
    assert!(
        success,
        "strict pre-projection source/N consumer: {response:?}"
    );
}

fn validate_owned(request: &Envelope, observed: &Observed) -> Result<(), String> {
    let unique = observed
        .roots
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if observed.semantic == [0; 32]
        || observed.n == [0; 32]
        || observed.n_len == 0
        || observed.constructed_work == 0
        || observed.replay_work == 0
        || observed.stage_receipt == 0
        || observed.unrelated_floor != FLOOR
        || observed.unrelated_floor.checked_add(observed.stage_receipt) != Some(observed.live_floor)
        || !observed.undercut_refused_before_work
        || observed.roots.len() != request.capture.case.roots()
        || unique.len() != observed.roots.len()
        || observed.rows.len() != observed.roots.len()
    {
        return Err("incomplete owning source/N/ranked/ledger observation".into());
    }
    for (root, row) in observed.roots.iter().zip(&observed.rows) {
        if *root != row.root
            || row.root_identity == [0; 32]
            || row.body_identity == [0; 32]
            || row.report_semantic != observed.semantic
            || row.ranked_sha256 == [0; 32]
            || row.ranked_len == 0
        {
            return Err("source/ranked/report root association changed".into());
        }
        if request.capture.case == Case::U64 {
            if row.certificates != 0 || row.consumed != (Consumed::NoU32Credit {}) {
                return Err("U64 no-loop control acquired U32 credit".into());
            }
        } else if row.certificates != 1
            || row.checked_additions == 0
            || !matches!(row.consumed, Consumed::SourceAndN { function, certificate: 0,
                dynamic_u32_bound: true, .. } if function == row.body)
        {
            return Err("strict U32 positive lacks actual consumed source/N progress".into());
        }
    }
    Ok(())
}
fn decode_owned(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    request: &Envelope,
) -> Result<Observed, String> {
    if status != Some(0) {
        return Err(format!("strict guarded child status {status:?}"));
    }
    let response: Response = serde_json::from_slice(bytes.ok_or("missing fresh guarded report")?)
        .map_err(|error| error.to_string())?;
    if response.protocol != PROTOCOL || response.request != *request {
        return Err("foreign guarded response".into());
    }
    let observed = response
        .result
        .map_err(|error| format!("{:?}: {}", error.stage, error.detail))?;
    validate_owned(request, &observed)?;
    Ok(observed)
}

#[test]
#[ignore = "strict owning pre-projection ordinary-Rust guard consumption; pinned nightly and AMD dependencies"]
fn ordinary_rust_guarded_source_stage_consumes_all_u32_sites_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-guarded-loop-source");
    let source = stamps(&workspace);
    let mut configurations = 0;
    let mut roots = 0;
    let mut consumed = 0;
    let mut no_credit = 0;
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
            let request = Envelope {
                protocol: PROTOCOL,
                capture: Request {
                    case,
                    target: target.into(),
                    args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
                    source: source.clone(),
                },
            };
            let args = directory.join("args.json");
            let report = directory.join("guarded-result.json");
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
                .env_remove(super::REQUEST)
                .env(CHILD_ARGS, &args)
                .env(CHILD_RESULT, &report)
                .env(NEW_REQUEST, serde_json::to_string(&request).unwrap())
                .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"]);
            progress::clear_inherited_jobserver(&mut command);
            fixed_census_observation::configure(&mut command, None);
            let output = command.output().unwrap();
            let bytes = std::fs::read(&report);
            let observed = decode_owned(output.status.code(), bytes.as_deref().ok(), &request)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} {target}: {error}\n{}",
                        case.feature(),
                        corpus_cargo::diagnostics(&output)
                    )
                });
            configurations += 1;
            roots += observed.roots.len();
            consumed += observed
                .rows
                .iter()
                .filter(|row| matches!(row.consumed, Consumed::SourceAndN { .. }))
                .count();
            no_credit += observed
                .rows
                .iter()
                .filter(|row| row.consumed == (Consumed::NoU32Credit {}))
                .count();
            assert_eq!(stamps(&workspace), source);
            eprintln!(
                "GUARDED SOURCE {} {target}: roots={}, consumed={}, original N={:02x?}; no native/default authority",
                case.feature(),
                observed.roots.len(),
                observed
                    .rows
                    .iter()
                    .filter(|row| matches!(row.consumed, Consumed::SourceAndN { .. }))
                    .count(),
                observed.n
            );
        }
    }
    assert_eq!(
        (configurations, roots, consumed, no_credit),
        (10, 12, 10, 2)
    );
}
