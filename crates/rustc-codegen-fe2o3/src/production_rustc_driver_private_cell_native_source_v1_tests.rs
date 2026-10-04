//! Ordinary Rust into the genuine unnumbered owning native stage. This test
//! needs the real admitted reference runtime; missing proof is never success.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, MemoryEffect, Module, Operation,
};

const REQUEST: &str = "FE2O3_TEST_PRIVATE_CELL_NATIVE_SOURCE_V1";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::private_cell_native_source::private_cell_native_source_child";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Case {
    Mutating,
    Noop,
    UnitLocal,
}
impl Case {
    fn feature(self) -> &'static str {
        match self {
            Self::Mutating => "private-cell-native",
            Self::Noop => "private-cell-native-noop",
            Self::UnitLocal => "private-cell-native-unitlocal",
        }
    }
    fn mutates(self) -> bool {
        self != Self::Noop
    }
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn profile(target: &str) -> Result<Profile, String> {
    match target {
        "gfx942" => Ok(Profile::Gfx942),
        "gfx950" => Ok(Profile::Gfx950),
        _ => Err("closed source-test target".into()),
    }
}
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn source_stamps() -> Vec<(String, [u8; 32])> {
    [
        "Cargo.lock".to_owned(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/private_cell_native.rs"),
    ]
    .into_iter()
    .map(|name| {
        let stamp = digest(&std::fs::read(workspace().join(&name)).unwrap());
        (name, stamp)
    })
    .collect()
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    case: Case,
    target: String,
    args_sha256: [u8; 32],
    sources: Vec<(String, [u8; 32])>,
}
impl Request {
    fn check(&self, args: &[String]) -> Result<(), String> {
        profile(&self.target)?;
        if self.args_sha256 != digest(&serde_json::to_vec(args).map_err(|e| e.to_string())?)
            || self.sources != source_stamps()
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
            return Err("exact ordinary-source invocation/bytes/retention flags".into());
        }
        require_canonical_overflow_checks_v1(args)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    callback_count: usize,
    roots: Vec<String>,
    unit_local: bool,
    selected: usize,
    historical_private: [usize; 3],
    promoted_private: [usize; 3],
    external_operations: usize,
    original: [u8; 32],
    historical_k: [u8; 32],
    promoted: [u8; 32],
    llvm_sha256: [u8; 32],
    llvm_bytes: usize,
    entry_work: usize,
    prepared_work: usize,
    replay_work: usize,
    retained_floor: usize,
    original_sim: simulation::SimulationObservation,
    promoted_sim: simulation::SimulationObservation,
}
impl Report {
    fn check(&self, request: &Request) -> Result<(), String> {
        if &self.request != request
            || self.callback_count != 1
            || self.roots != ["private_cell_native"]
            || self.unit_local != (request.case == Case::UnitLocal)
            || (self.selected != 0) != request.case.mutates()
            || (self.historical_k != self.promoted) != request.case.mutates()
            || self.historical_private[0].checked_sub(self.promoted_private[0])
                != Some(self.selected)
            || self.promoted_private[1] > self.historical_private[1]
            || self.promoted_private[2] > self.historical_private[2]
            || (request.case.mutates() && self.promoted_private[2] >= self.historical_private[2])
            || self.external_operations == 0
            || self.llvm_bytes == 0
            || self.original == [0; 32]
            || self.historical_k == [0; 32]
            || self.promoted == [0; 32]
            || self.llvm_sha256 == [0; 32]
            || self.entry_work != 7
            || self.prepared_work <= self.entry_work
            || self.replay_work == 0
            || self.retained_floor <= 37
        {
            return Err("actual source/promoted native custody or exact mode expectation".into());
        }
        simulation::check_report(
            &self.original_sim,
            self.original,
            simulation::Case::ScalarBorrow,
        )
        .map_err(|e| format!("original source SIM: {e:?}"))?;
        simulation::check_report(
            &self.promoted_sim,
            self.promoted,
            simulation::Case::ScalarBorrow,
        )
        .map_err(|e| format!("actual promoted SIM: {e:?}"))?;
        compare_simulations(&self.original_sim, &self.promoted_sim)
    }
}

fn compare_simulations(
    left: &simulation::SimulationObservation,
    right: &simulation::SimulationObservation,
) -> Result<(), String> {
    let scenario_rows = |report| -> Result<Vec<serde_json::Value>, String> {
        let value = serde_json::to_value(report).map_err(|e| e.to_string())?;
        let mut rows = value
            .get("scenarios")
            .and_then(serde_json::Value::as_array)
            .ok_or("typed SIM scenario rows")?
            .clone();
        if rows.len() != 6 {
            return Err("all six independent guard-buffer scenarios".into());
        }
        for row in &mut rows {
            // Promotion changes interpreter steps, not the byte oracle or launch.
            row.as_object_mut()
                .ok_or("typed SIM row")?
                .remove("steps")
                .ok_or("typed SIM step count")?;
        }
        Ok(rows)
    };
    if scenario_rows(left)? != scenario_rows(right)? {
        return Err("original/promoted complete SIM scenarios differ".into());
    }
    Ok(())
}

fn operations(module: &Module) -> impl Iterator<Item = &Operation> {
    module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
}
fn private_counts(module: &Module) -> [usize; 3] {
    let mut counts = [0; 3];
    for operation in operations(module) {
        match &operation.kind {
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            } => counts[0] += 1,
            OperationKind::Load { access, .. } | OperationKind::GuardedLoad { access, .. }
                if access.address_space == AddressSpace::Private =>
            {
                counts[1] += 1
            }
            OperationKind::Store { access, .. } | OperationKind::GuardedStore { access, .. }
                if access.address_space == AddressSpace::Private =>
            {
                counts[2] += 1
            }
            _ => {}
        }
    }
    counts
}
fn external_operations(module: &Module) -> Vec<&Operation> {
    operations(module)
        .filter(|operation| {
            matches!(operation.kind, OperationKind::Call { .. })
                || operation.memory_effects().iter().any(|effect| {
                    !matches!(
                        effect,
                        MemoryEffect::Allocate(AddressSpace::Private)
                            | MemoryEffect::Read(AddressSpace::Private)
                            | MemoryEffect::Write(AddressSpace::Private)
                    )
                })
        })
        .collect()
}

struct SourceCallbacks {
    request: Request,
    calls: usize,
    result: Option<Result<Report, String>>,
}
impl Callbacks for SourceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            if self.calls != 1 {
                return Err("source callback must execute exactly once".into());
            }
            let ranked = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .verify_general_kernel_checks()
            .map_err(|e| format!("actual ranked admission: {e:?}"))?;
            let route = ranked.checked_output_source_policy_v1();
            let unit_local =
                route == fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::UnitLocal;
            if unit_local != (self.request.case == Case::UnitLocal) {
                return Err(format!("expected actual source route, observed {route:?}"));
            }
            let mut work = Work::new(
                usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                    .map_err(|e| e.to_string())?,
            );
            let mut budget = Budget::new(
                &mut work,
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            budget.charge_work(7).map_err(|e| e.to_string())?;
            budget.reserve_storage(37).map_err(|e| e.to_string())?;
            let ledger = budget.work_ledger_identity_v1();
            let entry_work = budget.work();
            let (stage, receipt) = ranked
                .lower_private_cell_native_with_budget_v1(&mut budget)
                .map_err(|e| format!("genuine source to actual promoted native stage: {e:?}"))?;
            if budget.storage() != 37 || budget.work_ledger_identity_v1() != ledger {
                return Err("owning source constructor changed entry ledger/floor".into());
            }
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(|e| e.to_string())?;
            let floor = budget.storage();
            if floor != stage.retained_storage_floor_v1()
                || stage.grants_artifact_or_launch_authority()
            {
                return Err("actual unnumbered stage reservation/authority".into());
            }
            let prepared_work = budget.work();
            stage
                .verify_equivalence(&mut budget)
                .map_err(|e| format!("fresh promoted native replay: {e:?}"))?;
            let replay_work = budget
                .work()
                .checked_sub(prepared_work)
                .ok_or("replay work regression")?;
            if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
                return Err("promoted replay lost live source/sibling reservation".into());
            }
            let (selected, retained_unit_local) = stage.source_test_promotion_v1();
            if retained_unit_local != unit_local {
                return Err("retained source route changed".into());
            }
            let historical = stage.source_test_historical_p8_output_v1();
            let before_effects = external_operations(historical.module());
            let after_effects = external_operations(stage.output().module());
            if before_effects != after_effects {
                return Err("actual external effects changed".into());
            }
            if !self.request.case.mutates()
                && historical.canonical().canonical_bytes()
                    != stage.output().canonical().canonical_bytes()
            {
                return Err("true no-op must preserve exact canonical bytes".into());
            }
            let target = profile(&self.request.target)?;
            let scratch = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES * 3;
            budget.reserve_storage(scratch).map_err(|e| e.to_string())?;
            let native = match target {
                Profile::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(stage.output()),
                Profile::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(stage.output()),
            }.map_err(|e| e.to_string())?;
            let expected = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&native)
                .map_err(|e| e.to_string())?;
            drop(native);
            if expected != stage.llvm_ir() || stage.llvm_ir().contains(".fe2o3.kd.v1") {
                return Err("actual promoted LLVM or unnumbered descriptor absence".into());
            }
            drop(expected);
            budget.release_storage(scratch).map_err(|e| e.to_string())?;
            let report = Report {
                request: self.request.clone(),
                callback_count: self.calls,
                roots: stage
                    .output()
                    .module()
                    .kernels
                    .iter()
                    .map(|k| k.id.as_str().to_owned())
                    .collect(),
                unit_local,
                selected,
                historical_private: private_counts(historical.module()),
                promoted_private: private_counts(stage.output().module()),
                external_operations: after_effects.len(),
                original: *stage.original().canonical().identity().digest(),
                historical_k: *historical.canonical().identity().digest(),
                promoted: *stage.output().canonical().identity().digest(),
                llvm_sha256: digest(stage.llvm_ir().as_bytes()),
                llvm_bytes: stage.llvm_ir().len(),
                entry_work,
                prepared_work,
                replay_work,
                retained_floor: floor,
                original_sim: simulation::observe(
                    stage.original().canonical(),
                    simulation::Case::ScalarBorrow,
                )
                .map_err(|e| format!("original source SIM: {e:?}"))?,
                promoted_sim: simulation::observe(
                    stage.output().canonical(),
                    simulation::Case::ScalarBorrow,
                )
                .map_err(|e| format!("actual promoted SIM: {e:?}"))?,
            };
            report.check(&self.request)?;
            drop((before_effects, after_effects));
            drop(stage);
            budget
                .release_storage(receipt.retained_storage())
                .map_err(|e| e.to_string())?;
            if budget.storage() != 37 || budget.work_ledger_identity_v1() != ledger {
                return Err("consumed native stage did not preserve sibling floor".into());
            }
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; parent supplies exact ordinary Cargo invocation and real protected runtime"]
fn private_cell_native_source_child() {
    let args: Vec<String> = serde_json::from_slice(
        &std::fs::read(env::var_os(CHILD_ARGS).expect("parent args")).unwrap(),
    )
    .unwrap();
    let request: Request =
        serde_json::from_str(&env::var(REQUEST).expect("parent request")).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        request.check(&args)?;
        let mut callbacks = SourceCallbacks {
            request: request.clone(),
            calls: 0,
            result: None,
        };
        rustc_driver::run_compiler(&args, &mut callbacks);
        request.check(&args)?;
        if callbacks.calls != 1 {
            return Err("genuine source callback count".into());
        }
        callbacks
            .result
            .ok_or("genuine source callback result missing")?
    }))
    .unwrap_or_else(|_| Err("actual source/native stage or proof runtime panicked".into()));
    let success = result.is_ok();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(env::var_os(CHILD_RESULT).expect("parent result path"))
        .unwrap()
        .write_all(&serde_json::to_vec(&result).unwrap())
        .unwrap();
    assert!(success, "ordinary source private-cell stage: {result:?}");
}

fn fixture(case: Case, target: &str) -> corpus::Fixture {
    let hash = |name: &str| {
        digest(&std::fs::read(workspace().join(name)).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let manifest = format!("{BASE}/Cargo.toml");
    corpus::Fixture {
        fixture_id: format!("{target}-{}", case.feature()),
        target: target.into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/private_cell_native.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.feature().into()],
            kernel_symbols: vec!["private_cell_native".into()],
        },
    }
}

#[derive(Serialize)]
struct ChildObservation<'a> {
    child_test: &'static str,
    ordinal: usize,
    request: &'a Request,
    report: &'a Report,
    stdout_bytes: usize,
    stdout_sha256: [u8; 32],
    stderr_bytes: usize,
    stderr_sha256: [u8; 32],
}
#[derive(Serialize)]
struct CompletedObservation {
    ordinal: usize,
    request: Request,
    observation_sha256: [u8; 32],
}
const OBSERVATION_PREFIX: &[u8] = b"FE2O3_PRIVATE_CELL_SOURCE_OBSERVATION ";
const STDOUT_BEGIN: &[u8] = b"FE2O3_PRIVATE_CELL_CAPTURED_STDOUT_BEGIN\n";
const STDERR_BEGIN: &[u8] = b"\nFE2O3_PRIVATE_CELL_CAPTURED_STDERR_BEGIN\n";
const CAPTURE_END: &[u8] = b"\nFE2O3_PRIVATE_CELL_CAPTURED_STREAMS_END\n";

// Formatting only: the parent checks process status and Report before writing.
// Captured streams are byte-exact but their original interleaving is unknown.
fn write_observation(
    output: &mut impl std::io::Write,
    ordinal: usize,
    request: &Request,
    report: &Report,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<CompletedObservation, String> {
    if &report.request != request {
        return Err("observation request/report identity".into());
    }
    let bytes = serde_json::to_vec(&ChildObservation {
        child_test: CHILD,
        ordinal,
        request,
        report,
        stdout_bytes: stdout.len(),
        stdout_sha256: digest(stdout),
        stderr_bytes: stderr.len(),
        stderr_sha256: digest(stderr),
    })
    .map_err(|e| e.to_string())?;
    for part in [
        OBSERVATION_PREFIX,
        bytes.as_slice(),
        b"\n",
        STDOUT_BEGIN,
        stdout,
        STDERR_BEGIN,
        stderr,
        CAPTURE_END,
    ] {
        output.write_all(part).map_err(|e| e.to_string())?;
    }
    output.flush().map_err(|e| e.to_string())?;
    Ok(CompletedObservation {
        ordinal,
        request: request.clone(),
        observation_sha256: digest(&bytes),
    })
}

fn write_completion(
    output: &mut impl std::io::Write,
    observations: &[CompletedObservation],
) -> Result<(), String> {
    let expected = [
        ("gfx942", Case::Mutating),
        ("gfx942", Case::Noop),
        ("gfx942", Case::UnitLocal),
        ("gfx950", Case::Mutating),
        ("gfx950", Case::Noop),
        ("gfx950", Case::UnitLocal),
    ];
    if observations.len() != expected.len()
        || observations.iter().zip(expected).enumerate().any(
            |(index, (observation, (target, case)))| {
                observation.ordinal != index + 1
                    || observation.request.target != target
                    || observation.request.case != case
            },
        )
    {
        return Err("exact ordered six-child completion roster".into());
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
        "child_test": CHILD, "completed": observations.len(), "observations": observations,
    }))
    .map_err(|e| e.to_string())?;
    output
        .write_all(b"FE2O3_PRIVATE_CELL_SOURCE_COMPLETE ")
        .map_err(|e| e.to_string())?;
    output.write_all(&bytes).map_err(|e| e.to_string())?;
    output.write_all(b"\n").map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())
}

#[test]
#[ignore = "requires pinned rust-src, AMD dependencies and real admitted protected Verus runtime; missing runtime is failure"]
fn ordinary_rust_private_cell_native_three_modes_both_profiles() {
    let workspace = workspace();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-private-cell-native-source");
    let mut completed = 0;
    let mut observations = Vec::new();
    for target in ["gfx942", "gfx950"] {
        let target_dir = scratch.path().join(target);
        for case in [Case::Mutating, Case::Noop, Case::UnitLocal] {
            let fixture = fixture(case, target);
            let directory = scratch.path().join(&fixture.fixture_id);
            std::fs::create_dir(&directory).unwrap();
            let mut captured =
                corpus_cargo::capture(&workspace, &fixture, &directory, &target_dir).unwrap();
            captured.args.push("-Zmir-opt-level=0".into());
            captured.args.push("-Zinline-mir=no".into());
            let request = Request {
                case,
                target: target.into(),
                args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
                sources: source_stamps(),
            };
            request.check(&captured.args).unwrap();
            let args_path = directory.join("args.json");
            let report_path = directory.join("result.json");
            std::fs::write(&args_path, serde_json::to_vec(&captured.args).unwrap()).unwrap();
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .env_clear()
                .envs(captured.environment)
                .current_dir(captured.cwd)
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .env_remove(CHILD_PROOF_PROBE)
                .env(CHILD_ARGS, &args_path)
                .env(CHILD_RESULT, &report_path)
                .env(REQUEST, serde_json::to_string(&request).unwrap())
                .args(["--exact", CHILD, "--ignored", "--nocapture"]);
            progress::clear_inherited_jobserver(&mut command);
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}: {}",
                fixture.fixture_id,
                corpus_cargo::diagnostics(&output)
            );
            let report: Result<Report, String> =
                serde_json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();
            let report = report.unwrap();
            report.check(&request).unwrap();
            let stderr = std::io::stderr();
            observations.push(
                write_observation(
                    &mut stderr.lock(),
                    completed + 1,
                    &request,
                    &report,
                    &output.stdout,
                    &output.stderr,
                )
                .unwrap(),
            );
            completed += 1;
        }
    }
    assert_eq!(completed, 6, "three actual source modes on both profiles");
    write_completion(&mut std::io::stderr().lock(), &observations).unwrap();
}

#[test]
fn source_observation_format_retains_typed_identity_and_exact_stream_bytes() {
    let request = Request {
        case: Case::Mutating,
        target: "gfx942".into(),
        args_sha256: [7; 32],
        sources: vec![("format-only.rs".into(), [8; 32])],
    };
    let inert_sim = || {
        serde_json::from_value(serde_json::json!({
            "case": "scalar-borrow", "native_output_digest": ([0u8; 32]),
            "simulator_digest": ([0u8; 32]), "simulator_wire_version": 12,
            "canonical_bytes": 0, "numerical_policy": "format-only", "scenarios": [],
        }))
        .unwrap()
    };
    let mut report = Report {
        request: request.clone(),
        callback_count: 0,
        roots: Vec::new(),
        unit_local: false,
        selected: 0,
        historical_private: [0; 3],
        promoted_private: [0; 3],
        external_operations: 0,
        original: [0; 32],
        historical_k: [0; 32],
        promoted: [0; 32],
        llvm_sha256: [0; 32],
        llvm_bytes: 0,
        entry_work: 0,
        prepared_work: 0,
        replay_work: 0,
        retained_floor: 0,
        original_sim: inert_sim(),
        promoted_sim: inert_sim(),
    };
    // An intentionally invalid report exercises formatting, never source success.
    assert!(report.check(&request).is_err());
    let stdout = b"stdout\n\0\xff";
    let stderr = b"stderr\r\n\xfe";
    let mut bytes = Vec::new();
    let observation = write_observation(&mut bytes, 1, &request, &report, stdout, stderr).unwrap();
    assert!(bytes.starts_with(OBSERVATION_PREFIX));
    let end = bytes.iter().position(|byte| *byte == b'\n').unwrap();
    let record = &bytes[OBSERVATION_PREFIX.len()..end];
    let value: serde_json::Value = serde_json::from_slice(record).unwrap();
    assert_eq!(value["child_test"], CHILD);
    assert_eq!(value["ordinal"], 1);
    assert_eq!(value["request"], serde_json::to_value(&request).unwrap());
    assert_eq!(value["report"], serde_json::to_value(&report).unwrap());
    assert_eq!(value["stdout_bytes"], stdout.len());
    assert_eq!(value["stderr_bytes"], stderr.len());
    assert_eq!(
        value["stdout_sha256"],
        serde_json::to_value(digest(stdout)).unwrap()
    );
    assert_eq!(
        value["stderr_sha256"],
        serde_json::to_value(digest(stderr)).unwrap()
    );
    let captured = [
        STDOUT_BEGIN,
        stdout.as_slice(),
        STDERR_BEGIN,
        stderr.as_slice(),
        CAPTURE_END,
    ]
    .concat();
    assert_eq!(&bytes[end + 1..], captured.as_slice());
    assert_eq!(observation.request, request);
    assert_eq!(observation.observation_sha256, digest(record));
    let mut full: &mut [u8] = &mut [];
    assert!(write_observation(&mut full, 1, &request, &report, stdout, stderr).is_err());

    report.request.args_sha256[0] ^= 1;
    let mut rejected = Vec::new();
    assert!(write_observation(&mut rejected, 1, &request, &report, stdout, stderr).is_err());
    assert!(rejected.is_empty());
    let mut partial = Vec::new();
    assert!(write_completion(&mut partial, &[observation]).is_err());
    assert!(partial.is_empty());

    let mut roster = Vec::new();
    for target in ["gfx942", "gfx950"] {
        for case in [Case::Mutating, Case::Noop, Case::UnitLocal] {
            let mut request = request.clone();
            request.target = target.into();
            request.case = case;
            report.request = request.clone();
            assert!(report.check(&request).is_err());
            let ordinal = roster.len() + 1;
            roster.push(
                write_observation(&mut Vec::new(), ordinal, &request, &report, stdout, stderr)
                    .unwrap(),
            );
        }
    }
    let mut completed = Vec::new();
    write_completion(&mut completed, &roster).unwrap();
    let prefix = b"FE2O3_PRIVATE_CELL_SOURCE_COMPLETE ";
    assert!(completed.starts_with(prefix));
    let completion: serde_json::Value = serde_json::from_slice(&completed[prefix.len()..]).unwrap();
    assert_eq!(completion["completed"], 6);
    assert_eq!(completion["child_test"], CHILD);
    assert_eq!(
        completion["observations"],
        serde_json::to_value(&roster).unwrap()
    );
    roster.swap(0, 1);
    let mut reordered = Vec::new();
    assert!(write_completion(&mut reordered, &roster).is_err());
    assert!(reordered.is_empty());
}

#[test]
fn source_mode_expectations_are_closed_and_noop_requires_zero_selection() {
    assert!(Case::Mutating.mutates() && Case::UnitLocal.mutates());
    assert!(!Case::Noop.mutates());
    assert_eq!(profile("gfx942").unwrap(), Profile::Gfx942);
    assert_eq!(profile("gfx950").unwrap(), Profile::Gfx950);
    assert!(profile("gfx900").is_err());
    assert!(serde_json::from_str::<Case>("\"Anything\"").is_err());
}

#[test]
fn source_request_refuses_changed_bytes_arguments_and_retention_flags() {
    let args = vec![
        "rustc".to_owned(),
        "-Zmir-opt-level=0".into(),
        "-Zinline-mir=no".into(),
        "-Coverflow-checks=on".into(),
    ];
    let request = Request {
        case: Case::Mutating,
        target: "gfx942".into(),
        args_sha256: digest(&serde_json::to_vec(&args).unwrap()),
        sources: source_stamps(),
    };
    request.check(&args).unwrap();
    let mut stale = request.clone();
    stale.sources[0].1[0] ^= 1;
    assert!(stale.check(&args).is_err());
    let mut changed = args.clone();
    changed.push("--cfg=foreign".into());
    assert!(request.check(&changed).is_err());
    for (index, replacement) in [
        (1, "-Zmir-opt-level=2"),
        (2, "-Zinline-mir=yes"),
        (3, "-Coverflow-checks=off"),
    ] {
        let mut changed = args.clone();
        changed[index] = replacement.into();
        let mut matching_digest = request.clone();
        matching_digest.args_sha256 = digest(&serde_json::to_vec(&changed).unwrap());
        assert!(matching_digest.check(&changed).is_err());
    }
}
