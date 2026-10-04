//! Actual captured Rust must supply signed source custody; components cannot substitute.
use super::*;
use fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as STORAGE;

const ENTRY_STAGE: &str = "ordinary-ranked-to-inert-final-f-v1";
const ENTRY_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::licm_native_source::refined_forwarding::inert_entry::ranked_inert_wire_source_child";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    stage: String,
    ordinal: usize,
    subject: Subject,
}
impl Request {
    fn check(&self, args: &[String]) -> Result<(), String> {
        if self.stage != ENTRY_STAGE || !(1..=8).contains(&self.ordinal) {
            return Err("exact ranked inert entry stage and ordinal".into());
        }
        self.subject.check(args)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Measurement {
    work: usize,
    peak: usize,
    floor: usize,
    failed_storage: Option<usize>,
    direct_work: Option<[usize; 2]>,
    error: Option<String>,
    wire: Option<[u8; 32]>,
    retained: usize,
}
impl Measurement {
    fn success(&self) -> bool {
        self.error.is_none()
            && self.direct_work.is_none()
            && self.failed_storage.is_none()
            && self.wire.is_some_and(|value| value != [0; 32])
            && self.retained != 0
            && self.peak >= self.floor
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticObservation {
    source: [u8; 32],
    preflight: [u8; 32],
    ranked: [u8; 32],
    execution: [u8; 32],
    original: [u8; 32],
    graphs: [[u8; 32]; 12],
    origins: [[u8; 32]; 2],
    fields: [[u8; 32]; 14],
    lengths: [usize; 14],
    native: [u8; 32],
    descriptor: [u8; 32],
    llvm: [u8; 32],
    llvm_bytes: usize,
    roots: usize,
    hoists: usize,
    splits: usize,
    forwards: usize,
    deleted: [usize; 2],
    authority: [bool; 2],
    scenarios: Vec<Scenario>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    callbacks: usize,
    construction: Measurement,
    construction_controls: [Measurement; 4],
    replay: Measurement,
    replay_controls: [Measurement; 4],
    restoring_faults: usize,
    semantic: SemanticObservation,
}

fn controls(full: &Measurement, rows: &[Measurement; 4]) -> Result<(), String> {
    if !full.success()
        || !rows[0].success()
        || !rows[2].success()
        || &rows[0] != full
        || &rows[2] != full
        || rows[1].floor != full.floor
        || rows[3].floor != full.floor
        || rows[1].wire.is_some()
        || rows[3].wire.is_some()
        || rows[1].retained != 0
        || rows[3].retained != 0
        || rows[1].direct_work != Some([full.work, full.work.checked_sub(1).ok_or("zero Work")?])
        || rows[1].work.checked_add(1) != Some(full.work)
        || rows[1].failed_storage.is_some()
        || rows[1].error.as_ref().is_none_or(|v| v.is_empty())
        || rows[3].error.as_ref().is_none_or(|v| v.is_empty())
        || rows[3].direct_work.is_some()
        || rows[3].failed_storage != Some(full.peak)
        || rows[3].peak >= full.peak
        || rows[3].work > full.work
    {
        return Err("exact success/Work-short and diagnostic Storage-short resource rows".into());
    }
    Ok(())
}

impl Report {
    fn check(&self, request: &Request) -> Result<(), String> {
        let s = &self.semantic;
        let changed = usize::from(request.subject.case.motion());
        if &self.request != request
            || request.stage != ENTRY_STAGE
            || !(1..=8).contains(&request.ordinal)
            || self.callbacks != 1
            || !self.construction.success()
            || self.construction.work <= 7
            || self.construction.floor <= 37
            || self.restoring_faults != 3
            || self.construction.peak
                < self
                    .construction
                    .floor
                    .checked_add(self.construction.retained)
                    .ok_or("construction floor overflow")?
            || self.replay.floor
                != self
                    .construction
                    .floor
                    .checked_add(self.construction.retained)
                    .ok_or("floor overflow")?
            || self.replay.retained != self.construction.retained
            || self.replay.wire != self.construction.wire
            || s.source != s.preflight
            || s.splits != changed
            || s.forwards != changed
            || (changed == 0 && s.hoists != 0)
            || (changed != 0 && s.hoists < 2)
            || (s.graphs[9] != s.graphs[10]) != (changed != 0)
            || (s.graphs[10] != s.graphs[11]) != (changed != 0)
            || s.deleted
                != if request.subject.case.unit_local() {
                    [1, 1]
                } else {
                    [0, 0]
                }
            || s.roots == 0
            || s.llvm_bytes == 0
            || s.authority != [false; 2]
            || [
                s.source,
                s.ranked,
                s.execution,
                s.original,
                s.native,
                s.descriptor,
                s.llvm,
            ]
            .contains(&[0; 32])
            || s.graphs.contains(&[0; 32])
            || s.origins.contains(&[0; 32])
            || s.fields.contains(&[0; 32])
            || s.fields[1] != s.native
            || s.fields[2] != s.descriptor
            || s.lengths.iter().enumerate().any(|(i, n)| i != 5 && *n == 0)
            || (s.lengths[5] != 0) != request.subject.case.unit_local()
        {
            return Err("complete signed actual-F entry/source/history/native report".into());
        }
        controls(&self.construction, &self.construction_controls)?;
        controls(&self.replay, &self.replay_controls)?;
        let mut index = 0;
        for length in LENGTHS {
            for control in CONTROLS {
                for bound in BOUNDS {
                    for repeat in 0..2 {
                        let row = s.scenarios.get(index).ok_or("missing entry SIM row")?;
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
                            return Err("exact entry four-graph SIM scenario".into());
                        }
                        index += 1;
                    }
                }
            }
        }
        if index != 144 || s.scenarios.len() != index {
            return Err("exact 144 entry SIM rows".into());
        }
        Ok(())
    }
}

struct Run {
    measurement: Measurement,
    semantic: Option<SemanticObservation>,
    replay: Option<(Measurement, [Measurement; 4])>,
}

fn run_factory<'tcx>(
    tcx: TyCtxt<'tcx>,
    request: &Request,
    work_cap: usize,
    storage_cap: usize,
    observe_source: bool,
) -> Result<Run, String> {
    let ranked = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .verify_general_kernel_checks()
    .map_err(|error| format!("actual ranked inert source prerequisite: {error:?}"))?;
    let policy = if request.subject.case.unit_local() {
        fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::UnitLocal
    } else {
        fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
    };
    assert_eq!(ranked.checked_output_source_policy_v1(), policy);
    let sibling = vec![0x63u8; 43];
    let sibling_bytes = std::mem::size_of_val(&sibling) + sibling.capacity();
    let mut work = Work::new(work_cap);
    let mut budget = Budget::new(&mut work, storage_cap);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(37 + sibling_bytes).unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = ranked.prepare_inert_refined_forwarding_wire_with_budget_v1(&mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let mut measurement = Measurement {
        work: budget.work(),
        peak: budget.peak_storage(),
        floor,
        failed_storage: budget.failed_storage(),
        direct_work: result
            .as_ref()
            .err()
            .and_then(|e| e.ranked_entry_test_direct_work_denial_v1()),
        error: result.as_ref().err().map(|e| format!("{e:?}")),
        wire: None,
        retained: 0,
    };
    let mut semantic = None;
    let mut replay = None;
    if let Ok((mut owner, receipt)) = result {
        measurement.retained = receipt.retained_storage();
        measurement.wire = Some(digest(owner.canonical_bytes()));
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(owner.retained_storage_floor_v1(), budget.storage());
        assert!(!owner.grants_artifact_or_launch_authority());
        assert!(!owner.authenticates_execution());
        if observe_source {
            semantic = Some(
                owner
                    .with_ranked_entry_test_observation_v1(&mut budget, |view| {
                        let source = view.source;
                        assert_eq!(source.profile, profile(&request.subject.target).unwrap());
                        assert_eq!(source.unit_local, request.subject.case.unit_local());
                        assert_eq!(source.source_identity, source.preflight_identity);
                        assert_eq!(view.history.limits.refinement, RefineLimits::default());
                        assert_eq!(view.history.limits.forwarding, ForwardLimits::default());
                        let old = sim::Input {
                            original: source.original,
                            historical: source.historical,
                            promoted: source.promoted,
                            input: source.preheaders,
                            output: source.licm,
                            origins: source.licm_origins,
                            launch: source.launch,
                            semantic: source.semantic,
                            kernels: source.licm_kernels,
                            profile: source.profile,
                        };
                        let hoists = sim::shape(&old, request.subject.case);
                        let graphs = Graphs {
                            original: source.original,
                            licm: source.licm,
                            refined: source.refined,
                            final_graph: source.final_graph,
                            refinement: source.refinement_origins,
                            forwarding: source.forwarding_origins,
                            launch: source.launch,
                        };
                        let (splits, forwards) = shape(&graphs, request.subject.case);
                        assert_eq!(
                            source.final_kernels.len(),
                            graphs.final_graph.module().kernels.len()
                        );
                        for (formal, kernel) in source
                            .final_kernels
                            .iter()
                            .zip(&graphs.final_graph.module().kernels)
                        {
                            assert_eq!(formal.kernel(), &kernel.id);
                            assert_eq!(formal.entry(), &kernel.entry);
                            assert!(formal.inter_invocation_conflicts().is_empty());
                        }
                        let h = view.history;
                        let p5 = h.prefix.prefix.prefix.prefix;
                        let history = [
                            p5.input,
                            p5.intermediate,
                            p5.stored,
                            p5.output,
                            h.prefix.prefix.prefix.output,
                            h.prefix.prefix.output,
                            h.prefix.output,
                            h.promoted,
                            h.preheaders,
                            h.licm,
                            h.refined,
                            h.output,
                        ];
                        assert_eq!(
                            history[9].canonical().canonical_bytes(),
                            graphs.licm.canonical().canonical_bytes()
                        );
                        assert_eq!(
                            history[10].canonical().canonical_bytes(),
                            graphs.refined.canonical().canonical_bytes()
                        );
                        assert_eq!(
                            history[11].canonical().canonical_bytes(),
                            graphs.final_graph.canonical().canonical_bytes()
                        );
                        assert_eq!(view.fields[1], view.native);
                        assert_eq!(view.fields[2], view.descriptor);
                        assert_eq!(*view.llvm_digest, digest(view.llvm.as_bytes()));
                        SemanticObservation {
                            source: source.source_identity,
                            preflight: source.preflight_identity,
                            ranked: source.ranked_identity,
                            execution: digest(source.execution),
                            original: *graphs.original.canonical().identity().digest(),
                            graphs: history.map(|g| *g.canonical().identity().digest()),
                            origins: [
                                digest(format!("{:?}", graphs.refinement).as_bytes()),
                                digest(format!("{:?}", graphs.forwarding).as_bytes()),
                            ],
                            fields: view.fields.map(digest),
                            lengths: view.fields.map(|b| b.len()),
                            native: digest(view.native),
                            descriptor: digest(view.descriptor),
                            llvm: *view.llvm_digest,
                            llvm_bytes: view.llvm.len(),
                            roots: view.roots,
                            hoists,
                            splits,
                            forwards,
                            deleted: [source.deleted_helpers.0, source.deleted_helpers.1],
                            authority: [
                                owner.grants_artifact_or_launch_authority(),
                                owner.authenticates_execution(),
                            ],
                            scenarios: observe(&graphs, request.subject.case),
                        }
                    })
                    .map_err(|error| format!("actual wire source observer: {error:?}"))?,
            );
            let measure = |w, p| {
                let mut work = Work::new(w);
                let mut b = Budget::new(&mut work, p);
                b.charge_work(17).unwrap();
                b.reserve_storage(owner.retained_storage_floor_v1())
                    .unwrap();
                let ledger = b.work_ledger_identity_v1();
                let result = owner.verify_equivalence(&mut b);
                assert_eq!(b.storage(), owner.retained_storage_floor_v1());
                assert!(b.work_ledger_identity_v1() == ledger);
                Measurement {
                    work: b.work(),
                    peak: b.peak_storage(),
                    floor: b.storage(),
                    failed_storage: b.failed_storage(),
                    direct_work: result
                        .as_ref()
                        .err()
                        .and_then(|e| e.ranked_entry_test_direct_work_denial_v1()),
                    error: result.as_ref().err().map(|e| format!("{e:?}")),
                    wire: result.is_ok().then(|| digest(owner.canonical_bytes())),
                    retained: if result.is_ok() {
                        receipt.retained_storage()
                    } else {
                        0
                    },
                }
            };
            let full = measure(work_limit(), STORAGE);
            let rows = [
                measure(full.work, STORAGE),
                measure(full.work - 1, STORAGE),
                measure(work_limit(), full.peak),
                measure(work_limit(), full.peak - 1),
            ];
            // P-1 remains diagnostic until a closed source run identifies its exact phase.
            controls(&full, &rows)?;
            replay = Some((full, rows));
            owner.ranked_entry_test_restoring_faults_v1(&mut budget);
        }
        assert_eq!(budget.storage(), floor + receipt.retained_storage());
        drop(owner);
        budget.release_storage(receipt.retained_storage()).unwrap();
    }
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x63; 43]);
    drop(sibling);
    budget.release_storage(sibling_bytes + 37).unwrap();
    assert_eq!(budget.storage(), 0);
    Ok(Run {
        measurement,
        semantic,
        replay,
    })
}

struct EntryCallbacks {
    request: Request,
    calls: usize,
    result: Option<Result<Report, String>>,
}
impl Callbacks for EntryCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            if self.calls != 1 {
                return Err("one genuine inert entry callback".into());
            }
            let full = run_factory(tcx, &self.request, work_limit(), STORAGE, true)?;
            if !full.measurement.success() {
                return Err(format!(
                    "actual signed ranked-to-wire prerequisite: {:?}",
                    full.measurement
                ));
            }
            let w = full.measurement.work;
            let p = full.measurement.peak;
            let rows = [
                run_factory(tcx, &self.request, w, STORAGE, false)?.measurement,
                run_factory(tcx, &self.request, w - 1, STORAGE, false)?.measurement,
                run_factory(tcx, &self.request, work_limit(), p, false)?.measurement,
                run_factory(tcx, &self.request, work_limit(), p - 1, false)?.measurement,
            ];
            let (replay, replay_controls) = full.replay.ok_or("missing actual wire replay")?;
            let report = Report {
                request: self.request.clone(),
                callbacks: self.calls,
                construction: full.measurement,
                construction_controls: rows,
                replay,
                replay_controls,
                restoring_faults: 3,
                semantic: full.semantic.ok_or("missing complete source observation")?,
            };
            report.check(&self.request)?;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires genuine captured compiler and admitted signed reference runtime"]
fn ranked_inert_wire_source_child() {
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(env::var_os(CHILD_ARGS).unwrap()).unwrap()).unwrap();
    let request: Request = serde_json::from_str(&env::var(REQUEST).unwrap()).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        request.check(&args)?;
        let mut callbacks = EntryCallbacks {
            request: request.clone(),
            calls: 0,
            result: None,
        };
        rustc_driver::run_compiler(&args, &mut callbacks);
        request.check(&args)?;
        if callbacks.calls != 1 {
            return Err("exact inert entry callback completion".into());
        }
        callbacks
            .result
            .ok_or("missing actual inert entry result")?
    }))
    .unwrap_or_else(|_| Err("actual signed-source entry callback panicked".into()));
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(bytes.len() <= 524_288);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(env::var_os(CHILD_RESULT).unwrap())
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    assert!(
        result.is_ok(),
        "genuine ordinary ranked inert entry: {result:?}"
    );
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    request: Request,
    report: [u8; 32],
    callbacks: usize,
    stdout_bytes: usize,
    stdout: [u8; 32],
    stderr_bytes: usize,
    stderr: [u8; 32],
}
fn complete_entry(rows: &[Completion]) -> Result<(), String> {
    let mut index = 0;
    for target in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            let row = rows.get(index).ok_or("missing inert entry completion")?;
            if row.request.stage != ENTRY_STAGE
                || row.request.ordinal != index + 1
                || row.request.subject.target != target
                || row.request.subject.case != case
                || row.callbacks != 1
                || row.report == [0; 32]
                || row.stdout_bytes == 0
                || row.stdout == [0; 32]
                || row.stderr == [0; 32]
                || row.stdout_bytes > 16 * 1024 * 1024
                || row.stderr_bytes > 16 * 1024 * 1024
            {
                return Err("exact ordered inert entry completion association".into());
            }
            index += 1;
        }
    }
    if rows.len() != index {
        return Err("extra inert entry completion".into());
    }
    Ok(())
}

fn write_entry(
    output: &mut impl Write,
    report: &Report,
    stdout: &[u8],
    stderr: &[u8],
) -> std::io::Result<Completion> {
    let row = Completion {
        request: report.request.clone(),
        report: digest(&serde_json::to_vec(report).unwrap()),
        callbacks: report.callbacks,
        stdout_bytes: stdout.len(),
        stdout: digest(stdout),
        stderr_bytes: stderr.len(),
        stderr: digest(stderr),
    };
    writeln!(
        output,
        "FE2O3_INERT_ENTRY_OBSERVATION {}",
        serde_json::to_string(&(report, &row)).unwrap()
    )?;
    for (name, bytes) in [("STDOUT", stdout), ("STDERR", stderr)] {
        writeln!(
            output,
            "FE2O3_INERT_ENTRY_{name}_BEGIN {} {}",
            report.request.ordinal,
            bytes.len()
        )?;
        output.write_all(bytes)?;
        writeln!(
            output,
            "\nFE2O3_INERT_ENTRY_{name}_END {}",
            report.request.ordinal
        )?;
    }
    Ok(row)
}

fn child(
    captured: &corpus_cargo::Captured,
    request: &Request,
    directory: &Path,
    output: &mut impl Write,
) -> Completion {
    let args_path = directory.join("inert-entry-args.json");
    let result_path = directory.join("inert-entry-result.json");
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
        .args(["--exact", ENTRY_CHILD, "--ignored", "--nocapture"]);
    progress::clear_inherited_jobserver(&mut command);
    let result = command.output().unwrap();
    assert!(result.stdout.len() <= 16 * 1024 * 1024 && result.stderr.len() <= 16 * 1024 * 1024);
    // Preserve bounded typed failure JSON before failing the parent on a child refusal.
    let bytes = match std::fs::metadata(&result_path) {
        Ok(metadata) => {
            assert!(metadata.len() <= 524_288);
            std::fs::read(&result_path).unwrap()
        }
        Err(_) => Vec::new(),
    };
    if !result.status.success() {
        writeln!(
            output,
            "FE2O3_INERT_ENTRY_REFUSAL {} {}",
            request.ordinal,
            String::from_utf8_lossy(&bytes)
        )
        .unwrap();
    }
    assert!(
        result.status.success(),
        "actual signed inert entry child: {}",
        corpus_cargo::diagnostics(&result)
    );
    let stdout = std::str::from_utf8(&result.stdout).expect("genuine libtest stdout");
    assert_eq!(
        stdout
            .matches("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;")
            .count(),
        1
    );
    assert_eq!(
        stdout
            .matches(&format!("test {ENTRY_CHILD} ... ok"))
            .count(),
        1
    );
    let report: Result<Report, String> = serde_json::from_slice(&bytes).unwrap();
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
    write_entry(output, &report, &result.stdout, &result.stderr).unwrap()
}

#[test]
#[ignore = "requires all eight genuine signed source entries and admitted reference runtime; missing prerequisites fail"]
fn ordinary_rust_ranked_inert_wire_same_graph_direct_unitlocal_both_profiles() {
    let root = workspace();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-ordinary-ranked-inert-wire");
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
            let request = Request {
                stage: ENTRY_STAGE.into(),
                ordinal: rows.len() + 1,
                subject,
            };
            rows.push(child(&captured, &request, &directory, &mut output));
        }
    }
    complete_entry(&rows).unwrap();
    writeln!(
        output,
        "FE2O3_INERT_ENTRY_COMPLETE {}",
        serde_json::to_string(&rows).unwrap()
    )
    .unwrap();
}

// Transport controls only; these records never enter a source or signed-owner constructor.
fn inert_entry_report() -> Report {
    let inherited = inert_report();
    let request = Request {
        stage: ENTRY_STAGE.into(),
        ordinal: 1,
        subject: inherited.request.subject,
    };
    let make = |floor, retained, work, peak| Measurement {
        work,
        peak,
        floor,
        failed_storage: None,
        direct_work: None,
        error: None,
        wire: Some([7; 32]),
        retained,
    };
    let construction = make(104, 200, 1000, 10000);
    let replay = make(304, 200, 2000, 11000);
    let controls = |full: &Measurement| {
        let mut work = full.clone();
        work.work -= 1;
        work.direct_work = Some([full.work, full.work - 1]);
        work.error = Some("inert Work transport control".into());
        work.wire = None;
        work.retained = 0;
        let mut storage = full.clone();
        storage.work -= 7;
        storage.peak -= 8;
        storage.failed_storage = Some(full.peak);
        storage.error = Some("inert Storage transport control, not observed phase".into());
        storage.wire = None;
        storage.retained = 0;
        [full.clone(), work, full.clone(), storage]
    };
    let mut graphs = [[1; 32]; 12];
    graphs[9] = inherited.graphs[1];
    graphs[10] = inherited.graphs[2];
    graphs[11] = inherited.graphs[3];
    let mut lengths = [1; 14];
    lengths[5] = usize::from(request.subject.case.unit_local());
    Report {
        request,
        callbacks: 1,
        construction_controls: controls(&construction),
        replay_controls: controls(&replay),
        construction,
        replay,
        restoring_faults: 3,
        semantic: SemanticObservation {
            source: inherited.source,
            preflight: inherited.preflight,
            ranked: inherited.ranked,
            execution: inherited.execution,
            original: inherited.graphs[0],
            graphs,
            origins: inherited.origins,
            fields: [[1; 32]; 14],
            lengths,
            native: [1; 32],
            descriptor: [1; 32],
            llvm: inherited.llvm,
            llvm_bytes: inherited.llvm_bytes,
            roots: 1,
            hoists: inherited.hoists,
            splits: inherited.splits,
            forwards: inherited.forwards,
            deleted: inherited.deleted,
            authority: [false; 2],
            scenarios: inherited.scenarios,
        },
    }
}

#[test]
fn ordinary_ranked_inert_wire_protocol_rejects_partial_identity_resource_and_semantic_rows() {
    let report = inert_entry_report();
    report.check(&report.request).unwrap();
    let mut bad = report.clone();
    bad.callbacks = 0;
    assert!(bad.check(&report.request).is_err());
    let mut bad = report.clone();
    bad.semantic.source[0] ^= 1;
    assert!(bad.check(&report.request).is_err());
    for index in 0..12 {
        let mut bad = report.clone();
        bad.semantic.graphs[index] = [0; 32];
        assert!(bad.check(&report.request).is_err());
    }
    for index in 0..14 {
        let mut bad = report.clone();
        bad.semantic.fields[index] = [0; 32];
        assert!(bad.check(&report.request).is_err());
    }
    for index in 0..4 {
        let mut bad = report.clone();
        bad.construction_controls[index].floor += 1;
        assert!(bad.check(&report.request).is_err());
        let mut bad = report.clone();
        bad.replay_controls[index].floor += 1;
        assert!(bad.check(&report.request).is_err());
    }
    let mut bad = report.clone();
    bad.semantic.authority[0] = true;
    assert!(bad.check(&report.request).is_err());
    let mut bad = report.clone();
    bad.semantic.scenarios.pop();
    assert!(bad.check(&report.request).is_err());
    let mut bad = report.clone();
    bad.semantic.scenarios[0].steps[3] = 0;
    assert!(bad.check(&report.request).is_err());
    let mut json = serde_json::to_value(&report).unwrap();
    json.as_object_mut().unwrap().insert(
        "source_success_without_proof".into(),
        serde_json::json!(true),
    );
    assert!(serde_json::from_value::<Report>(json).is_err());
}

#[test]
fn ordinary_ranked_inert_wire_protocol_binds_streams_and_complete_eight_child_roster() {
    let mut rows = Vec::new();
    for target in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            let old = inert_request(rows.len() + 1, target, case);
            rows.push(Completion {
                request: Request {
                    stage: ENTRY_STAGE.into(),
                    ordinal: old.ordinal,
                    subject: old.subject,
                },
                report: [1; 32],
                callbacks: 1,
                stdout_bytes: 1,
                stdout: [2; 32],
                stderr_bytes: 0,
                stderr: digest(b""),
            });
        }
    }
    complete_entry(&rows).unwrap();
    for index in 0..8 {
        let mut bad = rows.clone();
        bad.remove(index);
        assert!(complete_entry(&bad).is_err());
        let mut bad = rows.clone();
        bad[index].callbacks = 0;
        assert!(complete_entry(&bad).is_err());
    }
    let mut bad = rows.clone();
    bad.swap(0, 1);
    assert!(complete_entry(&bad).is_err());
    let mut bad = rows.clone();
    bad.push(rows[0].clone());
    assert!(complete_entry(&bad).is_err());
    let report = inert_entry_report();
    let mut bytes = Vec::new();
    let a = write_entry(&mut bytes, &report, b"source stdout", b"source stderr").unwrap();
    assert_eq!(a.stdout, digest(b"source stdout"));
    assert_eq!(a.stderr, digest(b"source stderr"));
    assert_eq!(a.report, digest(&serde_json::to_vec(&report).unwrap()));
    let b = write_entry(
        &mut Vec::new(),
        &report,
        b"changed stdout",
        b"source stderr",
    )
    .unwrap();
    assert_ne!(a.stdout, b.stdout);
    assert_eq!(a.stderr, b.stderr);
}
