//! Strict ordinary-source Policy6 qualification; no production pass selector.
use super::fixed_census_observation as census;
use super::*;
use crate::production_pipeline::fixed_checked_output_policy6_v1::FixedCheckedOutputProductionCompilationPolicy6V1 as Stage;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use simulation::integer_identity::{Batch, Integer, ROOTS};

#[path = "production_rustc_driver_integer_identity_graph_v1_tests.rs"]
mod graph;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::integer_identity_source_child";
const REQUEST: &str = "FE2O3_TEST_POLICY6_SOURCE_REQUEST_V1";
const ARTIFACT: &str = "FE2O3_TEST_POLICY6_SOURCE_ARTIFACT_V1";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Target {
    Gfx942,
    Gfx950,
}
impl Target {
    fn cpu(self) -> &'static str {
        match self {
            Self::Gfx942 => "gfx942",
            Self::Gfx950 => "gfx950",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Case {
    batch: Batch,
    opt0: bool,
    target: Target,
}
impl Case {
    fn name(self) -> String {
        format!(
            "{}-{}-opt{}",
            self.target.cpu(),
            self.batch.name(),
            if self.opt0 { "0" } else { "normal" }
        )
    }
    fn simulation(self) -> simulation::Case {
        simulation::Case::IntegerIdentity(self.batch)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Mode {
    Observe,
    Extract,
    ExtractCensus,
    MissingProof,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileStamp {
    path: PathBuf,
    sha256: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    case: Case,
    mode: Mode,
    args_sha256: [u8; 32],
    source: Vec<FileStamp>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    result: Result<Outcome, String>,
}

#[derive(Debug, Deserialize, Serialize)]
enum Outcome {
    Observed(Box<Observation6>),
    Extracted {
        llvm_sha256: [u8; 32],
        llvm_bytes: usize,
        source_roots: Vec<census::SourceRoot>,
    },
    MissingProof {
        retained_floor: usize,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct Observation6 {
    source_digest: [u8; 32],
    original_digest: [u8; 32],
    erased_digest: Option<[u8; 32]>,
    before_digest: [u8; 32],
    output_digest: [u8; 32],
    original_order: Vec<String>,
    output_order: Vec<String>,
    roots: Vec<graph::Root>,
    policy: u16,
    passes: Vec<String>,
    pass_changed: Vec<bool>,
    execution_sha256: [u8; 32],
    continuation_sha256: [u8; 32],
    replay_work: usize,
    simulation: simulation::SimulationObservation,
    llvm_sha256: [u8; 32],
    llvm_bytes: usize,
    descriptor_roots: usize,
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn canonical_v12_digest(bytes: &[u8]) -> [u8; 32] {
    use fe2o3_kernel_ir::{
        VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1 as DOMAIN,
        VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_POLICY_V1 as POLICY,
    };
    let mut hash = Sha256::new();
    hash.update(u32::try_from(DOMAIN.len()).unwrap().to_le_bytes());
    hash.update(DOMAIN);
    hash.update(POLICY.to_le_bytes());
    hash.update(u64::try_from(bytes.len()).unwrap().to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

fn source_stamps(workspace: &Path) -> Vec<FileStamp> {
    [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/integer_identity.rs"),
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

fn active_fixture_hash(source: &[FileStamp]) -> Result<[u8; 32], String> {
    let exact_suffix = PathBuf::from(format!("{BASE}/src/integer_identity.rs"));
    let matching = source
        .iter()
        .filter(|row| row.path.ends_with(&exact_suffix))
        .collect::<Vec<_>>();
    let [active] = matching.as_slice() else {
        return Err("source stamps lack one exact active integer-identity fixture".into());
    };
    Ok(active.sha256)
}

fn check_request(request: &Request, args: &[String]) -> Result<(), String> {
    if request.args_sha256 != digest(&serde_json::to_vec(args).map_err(|e| e.to_string())?)
        || request.source.len() != 4
    {
        return Err("Policy6 request lost exact captured arguments/source roster".into());
    }
    let unique = request
        .source
        .iter()
        .map(|row| &row.path)
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != request.source.len() {
        return Err("duplicate source stamp".into());
    }
    for row in &request.source {
        if digest(&std::fs::read(&row.path).map_err(|e| e.to_string())?) != row.sha256 {
            return Err(format!(
                "source changed during Policy6 qualification: {:?}",
                row.path
            ));
        }
    }
    Ok(())
}

fn replay_actual_continuation(stage: &Stage) -> Result<usize, String> {
    use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, check_canonical_kir_transition_v1};
    use fe2o3_pliron::PlironOptimizationPassV1 as Pass;
    let checked = stage.checked_output();
    let prefix = checked.intermediate_policy5();
    let continuation = checked.continuation();
    if checked.execution().policy_version() != 6
        || prefix.execution().policy_version() != 5
        || continuation.execution().policy_version() != 6
        || continuation
            .report()
            .passes()
            .iter()
            .map(|pass| pass.pass())
            .collect::<Vec<_>>()
            != [
                Pass::IntegerNeutralCanonicalization,
                Pass::DeadCodeElimination,
            ]
        || checked.grants_authority()
        || continuation.grants_authority()
        || !std::ptr::eq(stage.output(), checked.owner())
        || continuation.native_input_audit_bytes() != prefix.owner().canonical().canonical_bytes()
    {
        return Err("actual Policy6 fixed roster, O/I custody or authority changed".into());
    }
    let before = (
        digest(stage.semantic().canonical_encoding()),
        digest(stage.original_canonical_bytes()),
        digest(prefix.native_input_audit_bytes()),
        digest(prefix.owner().canonical().canonical_bytes()),
        digest(prefix.execution().canonical_bytes()),
        digest(checked.execution().canonical_bytes()),
    );
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let floor = stage.retained_storage_floor_v1();
    budget
        .reserve_storage(floor)
        .map_err(|e| format!("{e:?}"))?;
    let result = (|| -> Result<(), String> {
        continuation
            .execution()
            .check_against(
                prefix.owner(),
                stage.output(),
                continuation.report(),
                continuation.map(),
                &mut budget,
            )
            .map_err(|e| format!("fixed execution replay: {e:?}"))?;
        continuation
            .map()
            .check_against(prefix.owner(), stage.output(), &mut budget)
            .map_err(|e| format!("actual O/I map replay: {e:?}"))?;
        let (source, source_storage) = CanonicalKirInventoryV1::derive(prefix.owner(), &mut budget)
            .map_err(|e| format!("actual O inventory: {e:?}"))?;
        budget
            .reserve_storage(source_storage.retained_storage())
            .map_err(|e| format!("{e:?}"))?;
        let (output, output_storage) = CanonicalKirInventoryV1::derive(stage.output(), &mut budget)
            .map_err(|e| format!("actual I inventory: {e:?}"))?;
        budget
            .reserve_storage(output_storage.retained_storage())
            .map_err(|e| format!("{e:?}"))?;
        let (_witness, storage) = check_canonical_kir_transition_v1(
            &source,
            &output,
            continuation.occurrences().candidate(),
            &mut budget,
        )
        .map_err(|e| format!("independent actual O/I complete occurrence replay: {e:?}"))?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(|e| format!("{e:?}"))?;
        Ok(())
    })();
    let extra = budget
        .storage()
        .checked_sub(floor)
        .ok_or("test replay released inherited floor")?;
    budget
        .release_storage(extra)
        .map_err(|e| format!("{e:?}"))?;
    result?;
    assert_eq!(budget.storage(), floor);
    stage
        .exercise_final_receipt_component_v1(&mut budget)
        .map_err(|e| format!("actual unsigned final-I receipt component: {e:?}"))?;
    assert_eq!(budget.storage(), floor);
    let replay_work = budget.work();
    assert!(replay_work > 0);
    let after = (
        digest(stage.semantic().canonical_encoding()),
        digest(stage.original_canonical_bytes()),
        digest(prefix.native_input_audit_bytes()),
        digest(prefix.owner().canonical().canonical_bytes()),
        digest(prefix.execution().canonical_bytes()),
        digest(checked.execution().canonical_bytes()),
    );
    if before != after {
        return Err("independent replay mutated source N or retained prefix".into());
    }
    Ok(replay_work)
}

fn probe_missing_proof(stage: Stage) -> Result<Outcome, String> {
    use crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1;
    use crate::production_pipeline::{
        ProductionPipelineError as Error,
        checked_output_policy6_v1::CheckedOutputPolicy6StageErrorV1 as StageError,
    };
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let floor = stage.retained_storage_floor_v1();
    budget
        .reserve_storage(floor)
        .map_err(|e| format!("{e:?}"))?;
    let result = stage.probe_native_source_lineage_v1(&mut budget);
    if budget.storage() != floor {
        return Err("missing-proof probe changed inherited floor".into());
    }
    match result {
        Err(Error::CheckedOutputPolicy6Stage(StageError::NativeSource(error)))
            if matches!(
                *error,
                NativeSourceLineageErrorV1::MissingSignedRankedReceipt { .. }
            ) =>
        {
            Ok(Outcome::MissingProof {
                retained_floor: floor,
            })
        }
        Err(error) => Err(format!("wrong native proof refusal: {error:?}")),
        Ok(()) => Err("unsigned Policy6 source acquired native proof custody".into()),
    }
}

fn observe(tcx: TyCtxt<'_>, request: &Request, artifact: &Path) -> Result<Outcome, String> {
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| format!("source collection: {e}"))?;
    let ranked = transaction
        .verify_general_kernel_checks()
        .map_err(|e| format!("ranked checks: {e:?}"))?;
    if !ranked.all_kernel_checks_are_clean() || ranked.grants_artifact_or_launch_authority() {
        return Err("actual ranked source checks/authority changed".into());
    }
    let stage = ranked
        .lower_fixed_checked_output_policy6_v1()
        .map_err(|e| format!("Policy6: {e:?}"))?;
    if stage.kernels().len() != ROOTS.len() {
        return Err("Policy6 fresh formal-kernel roster differs from exact source roots".into());
    }
    if request.mode == Mode::MissingProof {
        return probe_missing_proof(stage);
    }
    let roots = graph::observe(&stage, request.case)?;
    let replay_work = replay_actual_continuation(&stage)?;
    let before_digest = *stage
        .checked_output()
        .intermediate_policy5()
        .owner()
        .canonical()
        .identity()
        .digest();
    let output_digest = *stage.output().canonical().identity().digest();
    if request.case.opt0 && before_digest == output_digest {
        return Err("opt0 requires genuine Policy6 O-to-I mutation, not pre-folded MIR".into());
    }
    let passes = stage.checked_output().continuation().report().passes();
    if request.case.opt0 && !passes[0].changed() {
        return Err("actual surviving identities did not change in the integer pass".into());
    }
    let mut report = Observation6 {
        source_digest: *stage.semantic().semantic_sha256().as_bytes(),
        original_digest: stage.original_digest(),
        erased_digest: stage.erased_digest().copied(),
        before_digest,
        output_digest,
        original_order: graph::root_order(stage.original_module()),
        output_order: graph::root_order(stage.output().module()),
        roots,
        policy: stage.checked_output().execution().policy_version(),
        passes: passes
            .iter()
            .map(|pass| pass.pass().name().into())
            .collect(),
        pass_changed: passes.iter().map(|pass| pass.changed()).collect(),
        execution_sha256: digest(stage.checked_output().execution().canonical_bytes()),
        continuation_sha256: digest(
            stage
                .checked_output()
                .continuation()
                .execution()
                .canonical_bytes(),
        ),
        replay_work,
        simulation: simulation::observe(stage.output().canonical(), request.case.simulation())
            .map_err(|e| format!("{e:?}"))?,
        llvm_sha256: [0; 32],
        llvm_bytes: 0,
        descriptor_roots: 0,
    };
    if digest(stage.semantic().canonical_encoding()) != report.source_digest
        || canonical_v12_digest(stage.original_canonical_bytes()) != report.original_digest
    {
        return Err("source semantic or actual V12 N bytes differ from actual owner digest".into());
    }
    let (handoff, descriptor) = stage
        .into_worker_handoff_extraction_v1()
        .map_err(|e| format!("final-I extraction: {e:?}"))?;
    let target =
        fe2o3_compiler_ffi::DeviceTargetV1::parse(&format!("{}:xnack-", request.case.target.cpu()))
            .map_err(|e| format!("{e:?}"))?;
    if handoff.target() != target
        || handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
        || descriptor.grants_link_authority()
        || descriptor.grants_load_authority()
        || descriptor.grants_launch_authority()
        || descriptor.table().producer().version().as_str()
            != format!(
                "production-policy6-checked-{}-cov6-v1",
                request.case.target.cpu()
            )
        || descriptor.table().kernels().len() != ROOTS.len()
    {
        return Err("actual final-I native target/descriptor/authority changed".into());
    }
    graph::check_descriptor_bindings(&report.roots, descriptor.table().kernels())?;
    for root in descriptor.table().kernels() {
        let fe2o3_kernel_descriptor::BlockSizeV1::Exact(dimensions) = root.launch().block_size()
        else {
            return Err("identity descriptor lost exact declared launch".into());
        };
        if root.launch().rank() != 1
            || [dimensions.x(), dimensions.y(), dimensions.z()] != [64, 1, 1]
            || root.launch().max_flat_workgroup_size() != 64
        {
            return Err("identity descriptor declared launch changed".into());
        }
    }
    let llvm = std::str::from_utf8(handoff.module_bytes()).map_err(|e| e.to_string())?;
    graph::check_native(request.case, &report.roots, llvm)?;
    report.llvm_sha256 = digest(handoff.module_bytes());
    report.llvm_bytes = handoff.module_bytes().len();
    report.descriptor_roots = descriptor.table().kernels().len();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(artifact)
        .and_then(|mut file| file.write_all(handoff.module_bytes()))
        .map_err(|e| e.to_string())?;
    validate_observation(request.case, &report)?;
    Ok(Outcome::Observed(Box::new(report)))
}

struct Callbacks6 {
    request: Request,
    artifact: PathBuf,
    result: Option<Result<Outcome, String>>,
}
impl Callbacks for Callbacks6 {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(observe(tcx, &self.request, &self.artifact));
        Compilation::Stop
    }
}

fn validate_observation(case: Case, report: &Observation6) -> Result<(), String> {
    graph::exact_roster(&report.original_order)?;
    graph::validate_roots(case, &report.roots)?;
    if report.output_order != report.original_order
        || report.policy != 6
        || report.passes != ["integer-neutral-canonicalization", "dead-code-elimination"]
        || report.pass_changed.len() != 2
        || report.replay_work == 0
        || report.llvm_bytes == 0
        || report.descriptor_roots != ROOTS.len()
        || report.source_digest == [0; 32]
        || report.original_digest == [0; 32]
        || report.execution_sha256 == [0; 32]
        || report.continuation_sha256 == [0; 32]
        || (case.opt0 && (report.before_digest == report.output_digest || !report.pass_changed[0]))
    {
        return Err("Policy6 observation lost source/output/custody/execution evidence".into());
    }
    for row in &report.roots {
        if row.root != "identity_control" && row.after_binary_count != 0
            || row.root == "identity_control"
                && (row.before_binary_count != 1 || row.after_binary_count != 1)
            || case.opt0
                && ["identity_xor_zero", "identity_or_zero", "identity_and_ones"]
                    .contains(&row.root.as_str())
                && row.before_binary_count != 1
        {
            return Err("actual O/I identity/control evidence missing".into());
        }
    }
    simulation::check_report(&report.simulation, report.output_digest, case.simulation())
        .map_err(|e| format!("{e:?}"))
}

fn extract_with_source_observation(args: &[String], artifact: &Path) -> Result<Outcome, String> {
    use std::sync::{Arc, Mutex};
    let observed = Arc::new(Mutex::new((0_usize, None)));
    let state = Arc::clone(&observed);
    super::super::fixed_census_invocation_observer_v1_tests::with_observer(
        Box::new(move |_, semantic| {
            let mut state = state.lock().unwrap();
            state.0 += 1;
            state.1 = Some(census::roots(semantic));
        }),
        || run_production_fixed_checked_output_policy6_extraction_driver_v1(args, artifact),
    )?;
    let (count, roots) = Arc::try_unwrap(observed)
        .expect("invocation observer dropped")
        .into_inner()
        .unwrap();
    if count != 1 {
        return Err(format!(
            "expected one actual source observation in public fixed6 runner, got {count}"
        ));
    }
    let source_roots = roots.ok_or("public fixed6 source owner was not observed")??;
    let bytes = std::fs::read(artifact).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("real Policy6 extractor emitted empty output".into());
    }
    Ok(Outcome::Extracted {
        llvm_sha256: digest(&bytes),
        llvm_bytes: bytes.len(),
        source_roots,
    })
}

#[test]
#[ignore = "subprocess helper; parent supplies exact Cargo source, arguments and mode"]
fn integer_identity_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let request: Request = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    let artifact = PathBuf::from(env::var_os(ARTIFACT).unwrap());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<Outcome, String> {
            check_request(&request, &args)?;
            if artifact.exists() {
                return Err("Policy6 artifact path is not fresh".into());
            }
            let result = if matches!(request.mode, Mode::Extract | Mode::ExtractCensus) {
                extract_with_source_observation(&args, &artifact)?
            } else {
                let mut callbacks = Callbacks6 {
                    request: request.clone(),
                    artifact,
                    result: None,
                };
                rustc_driver::run_compiler(&args, &mut callbacks);
                callbacks
                    .result
                    .ok_or("actual Policy6 callback did not run")??
            };
            check_request(&request, &args)?;
            Ok(result)
        },
    ))
    .unwrap_or_else(|_| Err("rustc or actual Policy6 callback panicked".into()));
    let succeeded = result.is_ok();
    let report = Report { request, result };
    std::fs::write(
        env::var_os(CHILD_RESULT).unwrap(),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    assert!(
        succeeded,
        "strict actual Policy6 source qualification: {report:?}"
    );
}

fn decode_report(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    expected: &Request,
) -> Result<Outcome, String> {
    if status != Some(0) {
        return Err(format!(
            "strict Policy6 child exit must be zero: {status:?}"
        ));
    }
    let report: Report = serde_json::from_slice(bytes.ok_or("missing fresh Policy6 report")?)
        .map_err(|e| e.to_string())?;
    if &report.request != expected {
        return Err("Policy6 child report does not bind exact request".into());
    }
    let result = report.result?;
    if !matches!(
        (&result, expected.mode),
        (Outcome::Observed(_), Mode::Observe)
            | (Outcome::Extracted { .. }, Mode::Extract)
            | (Outcome::Extracted { .. }, Mode::ExtractCensus)
            | (Outcome::MissingProof { .. }, Mode::MissingProof)
    ) {
        return Err("Policy6 child substituted a different mode/outcome".into());
    }
    Ok(result)
}

fn cases() -> Vec<Case> {
    let mut result = Vec::new();
    for target in [Target::Gfx942, Target::Gfx950] {
        for integer in Integer::ALL {
            for retained in [false, true] {
                for opt0 in [false, true] {
                    result.push(Case {
                        batch: Batch { integer, retained },
                        opt0,
                        target,
                    });
                }
            }
        }
    }
    result
}

fn fixture(workspace: &Path, case: Case) -> corpus::Fixture {
    let hash = |relative: &str| {
        digest(&std::fs::read(workspace.join(relative)).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let manifest = format!("{BASE}/Cargo.toml");
    let mut features = vec![format!("integer-identity-{}", case.batch.integer.name())];
    if case.batch.retained {
        features.push("integer-identity-retained".into())
    }
    corpus::Fixture {
        fixture_id: case.name(),
        target: case.target.cpu().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/integer_identity.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features,
            kernel_symbols: ROOTS.map(str::to_owned).to_vec(),
        },
    }
}

fn child(
    captured: &corpus_cargo::Captured,
    directory: &Path,
    request: Request,
) -> (Outcome, PathBuf) {
    let directory = directory.join(format!("{:?}", request.mode));
    std::fs::create_dir(&directory).unwrap();
    let args = directory.join("args.json");
    let response = directory.join("result.json");
    let artifact = directory.join("output.ll");
    let census_path = directory.join("census.json");
    let census_id = census::run_id(&captured.args, &serde_json::to_string(&request).unwrap());
    std::fs::write(&args, serde_json::to_vec(&captured.args).unwrap()).unwrap();
    assert!(!response.exists() && !artifact.exists());
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .env_clear()
        .envs(captured.environment.iter().cloned())
        .current_dir(&captured.cwd)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove(CHILD_PROOF_PROBE)
        .env(CHILD_ARGS, &args)
        .env(CHILD_RESULT, &response)
        .env(ARTIFACT, &artifact)
        .env(REQUEST, serde_json::to_string(&request).unwrap())
        .args(["--exact", CHILD, "--ignored", "--nocapture"]);
    progress::clear_inherited_jobserver(&mut command);
    census::configure(
        &mut command,
        (request.mode == Mode::ExtractCensus)
            .then_some((census_path.as_path(), census_id.as_str())),
    );
    let output = command.output().unwrap();
    let bytes = std::fs::read(&response);
    let result = decode_report(output.status.code(), bytes.as_deref().ok(), &request)
        .unwrap_or_else(|e| {
            panic!(
                "{} {:?}: {e}\n{}",
                request.case.name(),
                request.mode,
                corpus_cargo::diagnostics(&output)
            )
        });
    if request.mode == Mode::ExtractCensus {
        let Outcome::Extracted { source_roots, .. } = &result else {
            unreachable!()
        };
        let report = census::read_report(&census_path).unwrap();
        census::check_header(
            &report,
            &captured.args,
            &captured.cwd,
            6,
            &format!("{}:xnack-", request.case.target.cpu()),
            &census_id,
            true,
        )
        .unwrap();
        census::check_selected(
            &report,
            source_roots,
            &[active_fixture_hash(&request.source).unwrap()],
        )
        .unwrap();
    } else {
        assert!(!census_path.exists(), "disabled census produced a report");
    }
    (result, artifact)
}

#[test]
fn census_requires_the_active_fixture_stamp_not_an_unrelated_known_file() {
    let active = FileStamp {
        path: PathBuf::from(format!("/workspace/{BASE}/src/integer_identity.rs")),
        sha256: [7; 32],
    };
    let unrelated = FileStamp {
        path: PathBuf::from(format!("/workspace/{BASE}/src/lib.rs")),
        sha256: [8; 32],
    };
    assert_eq!(
        active_fixture_hash(&[unrelated.clone(), active.clone()]),
        Ok([7; 32])
    );
    assert!(active_fixture_hash(&[unrelated]).is_err());
    assert!(active_fixture_hash(&[active.clone(), active]).is_err());
}

#[test]
fn policy6_matrix_has_exact_64_configurations_320_roots_and_32_mutation_cases() {
    let cases = cases();
    let unique = cases
        .iter()
        .map(|case| case.name())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        (cases.len(), unique.len(), cases.len() * ROOTS.len()),
        (64, 64, 320)
    );
    assert_eq!(cases.iter().filter(|case| case.opt0).count(), 32);
}

#[test]
fn policy6_child_protocol_rejects_status_missing_malformed_foreign_and_wrong_outcome() {
    let request = Request {
        case: cases()[0],
        mode: Mode::MissingProof,
        args_sha256: [7; 32],
        source: Vec::new(),
    };
    let good = serde_json::to_vec(&Report {
        request: request.clone(),
        result: Ok(Outcome::MissingProof { retained_floor: 3 }),
    })
    .unwrap();
    assert!(decode_report(Some(0), Some(&good), &request).is_ok());
    for status in [None, Some(1), Some(101), Some(134), Some(137)] {
        assert!(decode_report(status, Some(&good), &request).is_err());
    }
    for bytes in [None, Some(&b"not-json"[..]), Some(&b"{}"[..])] {
        assert!(decode_report(Some(0), bytes, &request).is_err());
    }
    for result in [
        Err("rustc setup/panic or ordinary source refusal".into()),
        Ok(Outcome::Extracted {
            llvm_sha256: [1; 32],
            llvm_bytes: 3,
            source_roots: Vec::new(),
        }),
    ] {
        let bad = serde_json::to_vec(&Report {
            request: request.clone(),
            result,
        })
        .unwrap();
        assert!(decode_report(Some(0), Some(&bad), &request).is_err());
    }
    for foreign in [
        Request {
            args_sha256: [8; 32],
            ..request.clone()
        },
        Request {
            case: cases()[1],
            ..request.clone()
        },
        Request {
            mode: Mode::Extract,
            ..request.clone()
        },
    ] {
        assert!(decode_report(Some(0), Some(&good), &foreign).is_err());
    }
}

#[test]
#[ignore = "strict genuine Cargo/AMD source Policy6 matrix; pinned nightly and reviewed source admission required"]
fn ordinary_rust_integer_identities_reach_final_i_native_and_sim_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-policy6-integer-source");
    let source = source_stamps(&workspace);
    let mut completed = 0;
    let mut roots = 0;
    let mut mutations = 0;
    let mut compiler_children = 0;
    for case in cases() {
        assert_eq!(source_stamps(&workspace), source);
        let directory = scratch.path().join(case.name());
        std::fs::create_dir(&directory).unwrap();
        let mut captured = corpus_cargo::capture(
            &workspace,
            &fixture(&workspace, case),
            &directory,
            &scratch.path().join(case.target.cpu()),
        )
        .unwrap();
        require_canonical_overflow_checks_v1(&captured.args).unwrap();
        if case.batch.retained {
            captured.args.push("-Zinline-mir=no".into())
        }
        if case.opt0 {
            captured.args.push("-Zmir-opt-level=0".into())
        }
        let request = Request {
            case,
            mode: Mode::Observe,
            source: source.clone(),
            args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
        };
        let (observed, native) = child(&captured, &directory, request.clone());
        compiler_children += 1;
        let Outcome::Observed(observed) = observed else {
            unreachable!()
        };
        validate_observation(case, &observed).unwrap();
        let (extracted, independent) = child(
            &captured,
            &directory,
            Request {
                mode: Mode::Extract,
                ..request.clone()
            },
        );
        let Outcome::Extracted {
            llvm_sha256,
            llvm_bytes,
            source_roots,
        } = extracted
        else {
            unreachable!()
        };
        compiler_children += 1;
        let actual = std::fs::read(native).unwrap();
        let emitted = std::fs::read(independent).unwrap();
        assert_eq!(actual, emitted, "independent real final-I extractor bytes");
        assert_eq!(digest(&actual), observed.llvm_sha256);
        assert_eq!(
            (llvm_sha256, llvm_bytes),
            (observed.llvm_sha256, observed.llvm_bytes)
        );
        let expected_roots = graph::census_roots(&observed.roots);
        let subjects = |roots: &[census::SourceRoot]| {
            roots
                .iter()
                .map(|root| (root.name.clone(), (root.function, root.body)))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        assert_eq!(subjects(&source_roots), subjects(&expected_roots));
        let (enabled, enabled_path) = child(
            &captured,
            &directory,
            Request {
                mode: Mode::ExtractCensus,
                ..request.clone()
            },
        );
        let Outcome::Extracted {
            llvm_sha256: enabled_digest,
            llvm_bytes: enabled_len,
            source_roots: enabled_roots,
        } = enabled
        else {
            unreachable!()
        };
        compiler_children += 1;
        assert_eq!(
            std::fs::read(enabled_path).unwrap(),
            emitted,
            "optional census changed actual public fixed6 extraction bytes"
        );
        assert_eq!((enabled_digest, enabled_len), (llvm_sha256, llvm_bytes));
        assert_eq!(
            enabled_roots, source_roots,
            "actual source owner changed with optional census"
        );
        let (proof, absent_artifact) = child(
            &captured,
            &directory,
            Request {
                mode: Mode::MissingProof,
                ..request
            },
        );
        let Outcome::MissingProof { retained_floor } = proof else {
            unreachable!()
        };
        compiler_children += 1;
        assert!(retained_floor > 0 && !absent_artifact.exists());
        assert_eq!(source_stamps(&workspace), source);
        completed += 1;
        roots += observed.roots.len();
        mutations += usize::from(case.opt0);
        eprintln!(
            "POLICY6 SOURCE {}: 5 roots; N={:02x?}; O={:02x?}; I={:02x?}; {} independently re-emitted bytes; exact enabled/disabled source census; exact missing-proof refusal",
            case.name(),
            observed.original_digest,
            observed.before_digest,
            observed.output_digest,
            observed.llvm_bytes
        );
    }
    assert_eq!(
        (completed, roots, mutations, compiler_children),
        (64, 320, 32, 256)
    );
}
