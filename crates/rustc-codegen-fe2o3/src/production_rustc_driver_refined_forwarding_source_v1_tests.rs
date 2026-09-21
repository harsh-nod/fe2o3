//! Genuine captured Rust through the complete combined owner, never Prefix6 fixtures.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingLimitsV1 as ForwardLimits,
    CanonicalKirInductionRefinementOriginV1 as Origin, CanonicalKirLoopLimitsV1 as RefineLimits,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Site,
    CheckedBinaryOperator, Constant, Operation, ScalarType,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationExecutionOutcomeV1, SimulationExecutionV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
};
use std::io::Write;

#[path = "production_rustc_driver_refined_forwarding_sim_v1_tests.rs"]
mod sim_combined;
use sim_combined::{guarded, observe};

const STAGE: &str = "ordinary-refinement-then-private-forwarding-v1";
const COMBINED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::licm_native_source::refined_forwarding::refined_forwarding_source_child";
const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
const LENGTHS: [usize; 6] = [0, 1, 63, 64, 65, 129];
const CONTROLS: [u32; 4] = [0, 1, 2, u32::MAX];
const BOUNDS: [u64; 3] = [0, 1, 3];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CombinedRequest {
    stage: String,
    ordinal: usize,
    subject: Subject,
}
impl CombinedRequest {
    fn check(&self, args: &[String]) -> Result<(), String> {
        if self.stage != STAGE || !(1..=8).contains(&self.ordinal) {
            return Err("exact combined stage and ordinal".into());
        }
        self.subject.check(args)
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    length: usize,
    control: u32,
    bound: u64,
    repeat: usize,
    invocations: [u64; 4],
    global_writes: [usize; 4],
    steps: [u64; 4],
    synthetic_false_pairs: usize,
    removed_reads: usize,
    backing: [u8; 32],
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CombinedReport {
    request: CombinedRequest,
    callbacks: usize,
    construction_work: usize,
    construction_peak: usize,
    incoming_floor: usize,
    retained: usize,
    source: [u8; 32],
    preflight: [u8; 32],
    ranked: [u8; 32],
    execution: [u8; 32],
    graphs: [[u8; 32]; 4],
    origins: [[u8; 32]; 2],
    llvm: [u8; 32],
    llvm_bytes: usize,
    hoists: usize,
    splits: usize,
    forwards: usize,
    deleted: [usize; 2],
    replay: [usize; 3],
    restoring_faults: usize,
    scenarios: Vec<Scenario>,
}
impl CombinedReport {
    fn check(&self, request: &CombinedRequest) -> Result<(), String> {
        let changed = usize::from(request.subject.case.motion());
        let final_charge = self
            .llvm_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or("replay charge overflow")?;
        let retained_floor = self
            .incoming_floor
            .checked_add(self.retained)
            .ok_or("retained floor overflow")?;
        if &self.request != request
            || request.stage != STAGE
            || !(1..=8).contains(&request.ordinal)
            || self.callbacks != 1
            || self.construction_work <= 7
            || self.construction_peak <= self.incoming_floor
            || self.incoming_floor <= 37
            || self.retained == 0
            || self.llvm_bytes == 0
            || self.source != self.preflight
            || self.splits != changed
            || self.forwards != changed
            || (changed == 0 && self.hoists != 0)
            || (changed != 0 && self.hoists < 2)
            || self.deleted
                != if request.subject.case.unit_local() {
                    [1, 1]
                } else {
                    [0, 0]
                }
            || (self.graphs[1] != self.graphs[2]) != (changed != 0)
            || (self.graphs[2] != self.graphs[3]) != (changed != 0)
            || self.restoring_faults != 3
            || self.replay[0] <= 17
            || self.replay[1] < retained_floor
            || self.replay[2]
                != self.replay[0]
                    .checked_sub(final_charge)
                    .ok_or("replay accounting")?
            || [self.source, self.ranked, self.execution, self.llvm].contains(&[0; 32])
            || self.graphs.contains(&[0; 32])
            || self.origins.contains(&[0; 32])
        {
            return Err("exact combined source owner, both rewrites and accounting".into());
        }
        let mut index = 0;
        for length in LENGTHS {
            for control in CONTROLS {
                for bound in BOUNDS {
                    for repeat in 0..2 {
                        let row = self
                            .scenarios
                            .get(index)
                            .ok_or("missing combined SIM row")?;
                        let active = length.min(64);
                        let trips = changed * active * usize::try_from(bound).unwrap();
                        let (_, expected) = guarded(length, true);
                        if (row.length, row.control, row.bound, row.repeat)
                            != (length, control, bound, repeat)
                            || row.invocations != [64; 4]
                            || row.steps.contains(&0)
                            || row.global_writes != [active; 4]
                            || row.synthetic_false_pairs != trips
                            || row.removed_reads != if control & 1 == 0 { trips } else { 0 }
                            || row.backing != digest(expected.buffer.bytes())
                        {
                            return Err("exact combined four-graph SIM scenario".into());
                        }
                        index += 1;
                    }
                }
            }
        }
        if index != 144 || self.scenarios.len() != index {
            return Err("exact 144 combined SIM rows".into());
        }
        Ok(())
    }
}

struct Graphs<'a> {
    original: &'a Graph,
    licm: &'a Graph,
    refined: &'a Graph,
    final_graph: &'a Graph,
    refinement: &'a [fe2o3_lower_mir_kernel::ProductionInductionRefinementOriginV1],
    forwarding: &'a [fe2o3_lower_mir_kernel::ProductionCrossBlockForwardingOriginV1],
    launch: &'a fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
}
fn operation(graph: &Graph, site: Site) -> &Operation {
    &graph.module().functions[site.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[site.block.block as usize]
        .operations[site.operation as usize]
}
fn count(graph: &Graph) -> usize {
    graph
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|body| &body.blocks)
        .map(|block| block.operations.len())
        .sum()
}
fn shape(value: &Graphs<'_>, case: Case) -> (usize, usize) {
    let l = value.licm;
    let r = value.refined;
    let f = value.final_graph;
    assert_eq!(l.module().kernels, r.module().kernels);
    assert_eq!(r.module().kernels, f.module().kernels);
    assert!(!std::ptr::eq(l, r) && !std::ptr::eq(r, f));
    assert_eq!(value.refinement.len(), count(l));
    let mut inputs = std::collections::BTreeSet::new();
    let mut outputs = std::collections::BTreeSet::new();
    let mut splits = 0;
    for row in value.refinement {
        assert_eq!(row.synthetic_false_source_statement(), None);
        match row.canonical_origin() {
            Origin::Unchanged { input, output } => {
                assert!(inputs.insert(input) && outputs.insert(output));
                assert_eq!(operation(l, input), operation(r, output));
            }
            Origin::CheckedAddSplit {
                input,
                sum_output,
                false_output,
                ..
            } => {
                splits += 1;
                assert!(
                    inputs.insert(input)
                        && outputs.insert(sum_output)
                        && outputs.insert(false_output)
                );
                assert!(row.original_source_statement().is_some());
                let old = operation(l, input);
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = old.kind
                else {
                    panic!("genuine ordinary source CheckedAdd")
                };
                assert_eq!(
                    operation(r, sum_output).kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs
                    }
                );
                assert_eq!(
                    operation(r, sum_output).results.as_slice(),
                    &old.results[..1]
                );
                assert_eq!(
                    operation(r, false_output).kind,
                    OperationKind::Constant(Constant::Bool(false))
                );
                assert_eq!(
                    operation(r, false_output).results.as_slice(),
                    &old.results[1..]
                );
                for site in [sum_output, false_output] {
                    let mut origins = value
                        .forwarding
                        .iter()
                        .filter(|row| row.canonical_origin().input == site);
                    let final_row = origins.next().unwrap();
                    assert!(origins.next().is_none());
                    assert_eq!(final_row.canonical_origin().store, None);
                    assert_eq!(operation(r, site), operation(f, site));
                    assert_eq!(
                        final_row.original_source_statement(),
                        if site == false_output {
                            None
                        } else {
                            row.original_source_statement()
                        }
                    );
                }
            }
        }
    }
    assert_eq!(outputs.len(), count(r));
    assert_eq!(value.forwarding.len(), count(r));
    assert_eq!(count(r), count(f));
    let mut coordinates = std::collections::BTreeSet::new();
    let mut selected = 0;
    for source in value.forwarding {
        let row = source.canonical_origin();
        assert!(coordinates.insert(row.input));
        assert_eq!(row.input, row.output);
        let old = operation(r, row.input);
        let new = operation(f, row.output);
        if let Some(store) = row.store {
            selected += 1;
            assert_ne!(store.block, row.input.block);
            let OperationKind::Load { pointer, access } = old.kind else {
                panic!("actual retained source Load")
            };
            let OperationKind::Store {
                pointer: destination,
                access: stored,
                value: stored_value,
            } = operation(r, store).kind
            else {
                panic!("actual selected R Store")
            };
            assert_eq!((pointer, access), (destination, stored));
            assert_eq!(access.address_space, AddressSpace::Private);
            assert_eq!(
                new.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: stored_value,
                    rhs: stored_value
                }
            );
            assert_eq!(new.results, old.results);
            let statement = source
                .original_source_statement()
                .expect("original selected Load statement");
            let stored_source = value
                .forwarding
                .iter()
                .find(|row| row.canonical_origin().input == store)
                .unwrap();
            assert_ne!(Some(statement), stored_source.original_source_statement());
        } else {
            assert_eq!(old, new);
        }
    }
    let expected = usize::from(case.motion());
    assert_eq!(
        (splits, selected),
        (expected, expected),
        "genuine source dual rewrite prerequisite, never no-op substitution"
    );
    assert_eq!(
        l.canonical().canonical_bytes() != r.canonical().canonical_bytes(),
        case.motion()
    );
    assert_eq!(
        r.canonical().canonical_bytes() != f.canonical().canonical_bytes(),
        case.motion()
    );
    (splits, selected)
}

struct CombinedCallbacks {
    request: CombinedRequest,
    calls: usize,
    result: Option<Result<CombinedReport, String>>,
}
impl Callbacks for CombinedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            if self.calls != 1 {
                return Err("one genuine combined callback".into());
            }
            let ranked = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .verify_general_kernel_checks()
            .map_err(|error| format!("actual combined source/ranked prerequisite: {error:?}"))?;
            let expected = if self.request.subject.case.unit_local() {
                fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::UnitLocal
            } else {
                fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
            };
            assert_eq!(ranked.checked_output_source_policy_v1(), expected);
            let sibling = vec![0x63u8; 43];
            let sibling_bytes = std::mem::size_of_val(&sibling) + sibling.capacity();
            let mut work = Work::new(work_limit());
            let report = {
                let mut budget = Budget::new(&mut work, storage_limit());
                budget.charge_work(7).unwrap();
                budget.reserve_storage(37 + sibling_bytes).unwrap();
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let result = ranked.lower_refined_cross_block_forwarding_native_with_budget_v1(
                    RefineLimits::default(),
                    ForwardLimits::default(),
                    &mut budget,
                );
                let construction_work = budget.work();
                let construction_peak = budget.peak_storage();
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                let (mut owner, receipt) = result.map_err(|error| {
                    format!("actual combined owning-entry prerequisite: {error:?}")
                })?;
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(owner.retained_storage_floor_v1(), budget.storage());
                assert_eq!(owner.refinement_limits(), RefineLimits::default());
                assert_eq!(owner.limits(), ForwardLimits::default());
                assert!(!owner.grants_artifact_or_launch_authority());
                owner
                    .verify_equivalence(&mut budget)
                    .map_err(|error| format!("full genuine combined wrapper replay: {error:?}"))?;
                let mut report = owner.with_source_test_observation_v1(|view| {
                    assert_eq!(view.profile, profile(&self.request.subject.target).unwrap());
                    assert_eq!(view.unit_local, self.request.subject.case.unit_local());
                    assert_eq!(view.source_identity, view.preflight_identity);
                    let old = sim::Input {
                        original: view.original,
                        historical: view.historical,
                        promoted: view.promoted,
                        input: view.preheaders,
                        output: view.licm,
                        origins: view.licm_origins,
                        launch: view.launch,
                        semantic: view.semantic,
                        kernels: view.licm_kernels,
                        profile: view.profile,
                    };
                    let hoists = sim::shape(&old, self.request.subject.case);
                    let value = Graphs {
                        original: view.original,
                        licm: view.licm,
                        refined: view.refined,
                        final_graph: view.final_graph,
                        refinement: view.refinement_origins,
                        forwarding: view.forwarding_origins,
                        launch: view.launch,
                    };
                    let (splits, forwards) = shape(&value, self.request.subject.case);
                    assert_eq!(
                        view.final_kernels.len(),
                        value.final_graph.module().kernels.len()
                    );
                    for (formal, kernel) in view
                        .final_kernels
                        .iter()
                        .zip(&value.final_graph.module().kernels)
                    {
                        assert_eq!(formal.kernel(), &kernel.id);
                        assert_eq!(formal.entry(), &kernel.entry);
                        assert!(formal.inter_invocation_conflicts().is_empty());
                    }
                    CombinedReport {
                        request: self.request.clone(),
                        callbacks: self.calls,
                        construction_work,
                        construction_peak,
                        incoming_floor: floor,
                        retained: receipt.retained_storage(),
                        source: view.source_identity,
                        preflight: view.preflight_identity,
                        ranked: view.ranked_identity,
                        execution: digest(view.execution),
                        graphs: [value.original, value.licm, value.refined, value.final_graph]
                            .map(|graph| *graph.canonical().identity().digest()),
                        origins: [
                            digest(format!("{:?}", value.refinement).as_bytes()),
                            digest(format!("{:?}", value.forwarding).as_bytes()),
                        ],
                        llvm: digest(owner.llvm_ir().as_bytes()),
                        llvm_bytes: owner.llvm_ir().len(),
                        hoists,
                        splits,
                        forwards,
                        deleted: [view.deleted_helpers.0, view.deleted_helpers.1],
                        replay: [0; 3],
                        restoring_faults: 0,
                        scenarios: observe(&value, self.request.subject.case),
                    }
                });
                report.replay =
                    owner.source_test_exact_replay_work_v1(work_limit(), storage_limit());
                owner.source_test_restoring_faults_v1(&mut budget);
                report.restoring_faults = 3;
                assert!(!owner.llvm_ir().contains(".fe2o3.kd.v1"));
                assert_eq!(budget.storage(), floor + receipt.retained_storage());
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(budget.failed_storage(), None);
                drop(owner);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(sibling, [0x63; 43]);
                drop(sibling);
                budget.release_storage(sibling_bytes + 37).unwrap();
                assert_eq!(budget.storage(), 0);
                report
            };
            assert_eq!(work.failed_work(), None);
            report.check(&self.request)?;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper requires genuine captured Cargo/rustc and admitted reference runtime"]
fn refined_forwarding_source_child() {
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(env::var_os(CHILD_ARGS).unwrap()).unwrap()).unwrap();
    let request: CombinedRequest = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        request.check(&args)?;
        let mut callbacks = CombinedCallbacks {
            request: request.clone(),
            calls: 0,
            result: None,
        };
        rustc_driver::run_compiler(&args, &mut callbacks);
        request.check(&args)?;
        if callbacks.calls != 1 {
            return Err("exact combined callback completion".into());
        }
        callbacks.result.ok_or("missing genuine combined result")?
    }))
    .unwrap_or_else(|_| Err("genuine combined source/runtime/admission callback panicked".into()));
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(bytes.len() <= 262_144);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(env::var_os(CHILD_RESULT).unwrap())
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    assert!(
        result.is_ok(),
        "genuine ordinary source combined entry: {result:?}"
    );
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CombinedCompletion {
    request: CombinedRequest,
    report_sha256: [u8; 32],
    callback_count: usize,
    stdout_bytes: usize,
    stdout_sha256: [u8; 32],
    stderr_bytes: usize,
    stderr_sha256: [u8; 32],
}
fn complete(rows: &[CombinedCompletion]) -> Result<(), String> {
    let mut index = 0;
    for target in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            let row = rows.get(index).ok_or("missing combined completion")?;
            if row.request.stage != STAGE
                || row.request.ordinal != index + 1
                || row.request.subject.target != target
                || row.request.subject.case != case
                || row.callback_count != 1
                || row.report_sha256 == [0; 32]
                || row.stdout_bytes == 0
                || row.stdout_sha256 == [0; 32]
                || row.stderr_sha256 == [0; 32]
                || row.stdout_bytes > 16 * 1024 * 1024
                || row.stderr_bytes > 16 * 1024 * 1024
            {
                return Err("exact ordered combined completion association".into());
            }
            index += 1;
        }
    }
    if rows.len() != index {
        return Err("extra combined completion".into());
    }
    Ok(())
}
fn write_combined_observation(
    output: &mut impl Write,
    report: &CombinedReport,
    stdout: &[u8],
    stderr: &[u8],
) -> std::io::Result<CombinedCompletion> {
    let report_bytes = serde_json::to_vec(report).map_err(std::io::Error::other)?;
    let completion = CombinedCompletion {
        request: report.request.clone(),
        report_sha256: digest(&report_bytes),
        callback_count: report.callbacks,
        stdout_bytes: stdout.len(),
        stdout_sha256: digest(stdout),
        stderr_bytes: stderr.len(),
        stderr_sha256: digest(stderr),
    };
    writeln!(
        output,
        "FE2O3_COMBINED_SOURCE_OBSERVATION {}",
        serde_json::to_string(&(report, &completion)).map_err(std::io::Error::other)?
    )?;
    writeln!(
        output,
        "FE2O3_COMBINED_STDOUT_BEGIN {} {}",
        report.request.ordinal,
        stdout.len()
    )?;
    output.write_all(stdout)?;
    writeln!(
        output,
        "\nFE2O3_COMBINED_STDOUT_END {}",
        report.request.ordinal
    )?;
    writeln!(
        output,
        "FE2O3_COMBINED_STDERR_BEGIN {} {}",
        report.request.ordinal,
        stderr.len()
    )?;
    output.write_all(stderr)?;
    writeln!(
        output,
        "\nFE2O3_COMBINED_STDERR_END {}",
        report.request.ordinal
    )?;
    Ok(completion)
}
fn combined_child(
    captured: &corpus_cargo::Captured,
    request: &CombinedRequest,
    directory: &Path,
    output: &mut impl Write,
) -> CombinedCompletion {
    let args_path = directory.join("combined-args.json");
    let result_path = directory.join("combined-result.json");
    std::fs::write(&args_path, serde_json::to_vec(&captured.args).unwrap()).unwrap();
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .env_clear()
        .envs(captured.environment.iter().cloned())
        .current_dir(&captured.cwd)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove(CHILD_PROOF_PROBE)
        .env(CHILD_ARGS, &args_path)
        .env(CHILD_RESULT, &result_path)
        .env(REQUEST, serde_json::to_string(request).unwrap())
        .args(["--exact", COMBINED_CHILD, "--ignored", "--nocapture"]);
    progress::clear_inherited_jobserver(&mut command);
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "actual combined source child: {}",
        corpus_cargo::diagnostics(&result)
    );
    assert!(result.stdout.len() <= 16 * 1024 * 1024 && result.stderr.len() <= 16 * 1024 * 1024);
    let stdout = std::str::from_utf8(&result.stdout).expect("genuine libtest stdout");
    assert_eq!(
        stdout
            .matches("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;")
            .count(),
        1
    );
    assert_eq!(
        stdout
            .matches(&format!("test {COMBINED_CHILD} ... ok"))
            .count(),
        1
    );
    assert!(std::fs::metadata(&result_path).unwrap().len() <= 262_144);
    let report: Result<CombinedReport, String> =
        serde_json::from_slice(&std::fs::read(result_path).unwrap()).unwrap();
    let report = report.unwrap();
    report.check(request).unwrap();
    assert_eq!(source_stamps(), request.subject.sources);
    assert_eq!(
        digest(&std::fs::read(env::current_exe().unwrap()).unwrap()),
        request.subject.test_binary_sha256
    );
    assert_eq!(
        digest(&std::fs::read(&captured.args[0]).unwrap()),
        request.subject.rustc_sha256
    );
    write_combined_observation(output, &report, &result.stdout, &result.stderr).unwrap()
}

#[test]
#[ignore = "requires genuine source dual rewrite admission and admitted reference runtime; missing prerequisites fail"]
fn ordinary_rust_refined_forwarding_native_same_graph_direct_unitlocal_both_profiles() {
    let root = workspace();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-ordinary-refined-forwarding");
    let stderr = std::io::stderr();
    let mut output = stderr.lock();
    let mut rows = Vec::new();
    for target in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            let fixture = fixture(case, target);
            let directory = scratch.path().join(&fixture.fixture_id);
            std::fs::create_dir(&directory).unwrap();
            let mut captured =
                corpus_cargo::capture(&root, &fixture, &directory, &scratch.path().join(target))
                    .unwrap();
            captured.args.push("-Zmir-opt-level=0".into());
            captured.args.push("-Zinline-mir=no".into());
            let subject = Subject {
                case,
                target: target.into(),
                cfg: captured.cfg.clone(),
                args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
                environment_sha256: environment_digest(captured.environment.clone()).unwrap(),
                cwd: captured.cwd.clone(),
                rustc_sha256: digest(&std::fs::read(&captured.args[0]).unwrap()),
                test_binary_sha256: digest(&std::fs::read(env::current_exe().unwrap()).unwrap()),
                sources: source_stamps(),
            };
            let request = CombinedRequest {
                stage: STAGE.into(),
                ordinal: rows.len() + 1,
                subject,
            };
            rows.push(combined_child(&captured, &request, &directory, &mut output));
        }
    }
    complete(&rows).unwrap();
    writeln!(
        output,
        "FE2O3_COMBINED_SOURCE_COMPLETE {}",
        serde_json::to_string(&rows).unwrap()
    )
    .unwrap();
}

// These records are deliberately inert transport controls, never source owners.
fn inert_request(ordinal: usize, target: &str, case: Case) -> CombinedRequest {
    CombinedRequest {
        stage: STAGE.into(),
        ordinal,
        subject: Subject {
            case,
            target: target.into(),
            args_sha256: [1; 32],
            cfg: vec![format!("feature=\"{}\"", case.feature())],
            environment_sha256: [2; 32],
            cwd: PathBuf::from("inert-not-a-source-invocation"),
            rustc_sha256: [3; 32],
            test_binary_sha256: [4; 32],
            sources: Vec::new(),
        },
    }
}
fn inert_report() -> CombinedReport {
    let request = inert_request(2, "gfx942", Case::DirectNoop);
    let mut scenarios = Vec::new();
    for length in LENGTHS {
        for control in CONTROLS {
            for bound in BOUNDS {
                for repeat in 0..2 {
                    let (_, expected) = guarded(length, true);
                    scenarios.push(Scenario {
                        length,
                        control,
                        bound,
                        repeat,
                        invocations: [64; 4],
                        global_writes: [length.min(64); 4],
                        steps: [1; 4],
                        synthetic_false_pairs: 0,
                        removed_reads: 0,
                        backing: digest(expected.buffer.bytes()),
                    });
                }
            }
        }
    }
    CombinedReport {
        request,
        callbacks: 1,
        construction_work: 1000,
        construction_peak: 700,
        incoming_floor: 100,
        retained: 200,
        source: [1; 32],
        preflight: [1; 32],
        ranked: [2; 32],
        execution: [3; 32],
        graphs: [[4; 32], [5; 32], [5; 32], [5; 32]],
        origins: [[6; 32]; 2],
        llvm: [7; 32],
        llvm_bytes: 1,
        hoists: 0,
        splits: 0,
        forwards: 0,
        deleted: [0, 0],
        replay: [1000, 700, 997],
        restoring_faults: 3,
        scenarios,
    }
}
#[test]
fn ordinary_refined_forwarding_protocol_keeps_closed_subject_graphs_and_resource_fields() {
    fn assert_send<T: Send>() {}
    assert_send::<CombinedCallbacks>();
    let report = inert_report();
    report.check(&report.request).unwrap();
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(
        serde_json::from_value::<CombinedReport>(value.clone()).unwrap(),
        report
    );
    for key in value.as_object().unwrap().keys() {
        let mut changed = value.clone();
        changed.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<CombinedReport>(changed).is_err(),
            "required field {key}"
        );
    }
    let mut unknown = value.clone();
    unknown["unrecognized"] = true.into();
    assert!(serde_json::from_value::<CombinedReport>(unknown).is_err());
    for field in ["stage", "subject"] {
        let mut wrong = report.request.clone();
        if field == "stage" {
            wrong.stage = "ordinary-licm-only".into();
        } else {
            wrong.subject.environment_sha256[0] ^= 1;
        }
        assert!(report.check(&wrong).is_err());
    }
    for mutate in [
        (|r: &mut CombinedReport| r.callbacks = 0) as fn(&mut CombinedReport),
        |r| r.preflight[0] ^= 1,
        |r| r.graphs[3][0] ^= 1,
        |r| r.splits = 1,
        |r| r.forwards = 1,
        |r| r.replay[2] += 1,
        |r| r.llvm_bytes = usize::MAX,
        |r| r.retained = usize::MAX,
        |r| r.scenarios[0].removed_reads = 1,
    ] {
        let mut changed = report.clone();
        mutate(&mut changed);
        assert!(changed.check(&changed.request).is_err());
    }
    let mut env_rows = env::vars_os().collect::<Vec<_>>();
    env_rows.retain(|(key, _)| {
        key.as_os_str() != std::ffi::OsStr::new("FE2O3_TEST_NONTRANSPORT_SENTINEL")
    });
    let before = environment_digest(env_rows.clone()).unwrap();
    env_rows.push(("FE2O3_TEST_NONTRANSPORT_SENTINEL".into(), "changed".into()));
    assert_ne!(before, environment_digest(env_rows).unwrap());
}
#[test]
fn ordinary_refined_forwarding_protocol_binds_raw_streams_and_exact_eight_child_roster() {
    let report = inert_report();
    let mut bytes = Vec::new();
    let stdout = b"inert\x00\xffstdout";
    let stderr = b"inert\x80stderr";
    let observation = write_combined_observation(&mut bytes, &report, stdout, stderr).unwrap();
    assert_eq!(
        (observation.stdout_bytes, observation.stderr_bytes),
        (stdout.len(), stderr.len())
    );
    assert_eq!(
        (observation.stdout_sha256, observation.stderr_sha256),
        (digest(stdout), digest(stderr))
    );
    assert!(bytes.windows(stdout.len()).any(|slice| slice == stdout));
    assert!(bytes.windows(stderr.len()).any(|slice| slice == stderr));
    struct RefuseWrite;
    impl Write for RefuseWrite {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("inert write refusal"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(write_combined_observation(&mut RefuseWrite, &report, stdout, stderr).is_err());
    let mut rows = Vec::new();
    for target in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            let mut row = observation.clone();
            row.request = inert_request(rows.len() + 1, target, case);
            rows.push(row);
        }
    }
    complete(&rows).unwrap();
    assert!(complete(&rows[..7]).is_err());
    let mut wrong = rows.clone();
    wrong.push(rows[0].clone());
    assert!(complete(&wrong).is_err());
    let mut wrong = rows.clone();
    wrong.swap(0, 1);
    assert!(complete(&wrong).is_err());
    let mut wrong = rows.clone();
    wrong[1] = rows[0].clone();
    assert!(complete(&wrong).is_err());
    let mut wrong = rows.clone();
    wrong[0].callback_count = 2;
    assert!(complete(&wrong).is_err());
    let mut wrong = rows;
    wrong[0].report_sha256 = [0; 32];
    assert!(complete(&wrong).is_err());
}
