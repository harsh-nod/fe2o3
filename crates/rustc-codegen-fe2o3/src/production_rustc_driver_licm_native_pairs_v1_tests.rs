//! Two actual sessions, one immutable Config context per captured invocation.
use super::context::{Context, PAIR_ARGS, PAIR_REQUEST, PAIR_RESULT, Record};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any, resume_unwind};

const PAIR_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::licm_native_source::pairs::licm_native_session_pair_child";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Mode {
    ContextOnly,
    NativeDirect,
    NativeUnitLocal,
}
impl Mode {
    fn cases(self) -> [Case; 2] {
        match self {
            Self::ContextOnly => [Case::DirectNoop, Case::UnitLocalNoop],
            Self::NativeDirect => [Case::DirectMotion, Case::DirectNoop],
            Self::NativeUnitLocal => [Case::UnitLocalMotion, Case::UnitLocalNoop],
        }
    }
    fn native(self) -> bool {
        self != Self::ContextOnly
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    ordinal: usize,
    mode: Mode,
    contexts: [Record; 2],
}
impl Request {
    fn check(&self) -> Result<(), String> {
        if self.ordinal == 0 {
            return Err("one-based pair ordinal".into());
        }
        for (record, case) in self.contexts.iter().zip(self.mode.cases()) {
            record.check()?;
            if record.case() != case {
                return Err("closed pair case roster".into());
            }
        }
        if self.contexts[0].target() != self.contexts[1].target()
            || self.contexts[0].identity() == self.contexts[1].identity()
            || self.contexts[0].binding() == self.contexts[1].binding()
            || self.contexts[0].observation() == self.contexts[1].observation()
        {
            return Err("genuinely distinct captured context subjects".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeIdentity {
    source: [u8; 32],
    semantic: [u8; 32],
    preflight: [u8; 32],
    ranked: [u8; 32],
    execution: [u8; 32],
    original: [u8; 32],
    historical: [u8; 32],
    promoted: [u8; 32],
    input: [u8; 32],
    output: [u8; 32],
    llvm: [u8; 32],
    hoists: usize,
    helpers: [usize; 2],
}
fn identity(
    input: &sim::Input<'_>,
    case: Case,
    (source, preflight, ranked): ([u8; 32], [u8; 32], [u8; 32]),
    execution: &[u8],
    llvm: &str,
    helpers: (usize, usize),
) -> NativeIdentity {
    assert_eq!(source, preflight);
    assert_eq!(helpers, if case.unit_local() { (1, 1) } else { (0, 0) });
    NativeIdentity {
        source,
        semantic: *input.semantic.semantic_sha256().as_bytes(),
        preflight,
        ranked,
        execution: digest(execution),
        original: *input.original.canonical().identity().digest(),
        historical: *input.historical.canonical().identity().digest(),
        promoted: *input.promoted.canonical().identity().digest(),
        input: *input.input.canonical().identity().digest(),
        output: *input.output.canonical().identity().digest(),
        llvm: digest(llvm.as_bytes()),
        hoists: sim::shape(input, case),
        helpers: [helpers.0, helpers.1],
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeObservation {
    identity: NativeIdentity,
    simulation: sim::Observation,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SessionObservation {
    context: [u8; 32],
    configs: usize,
    callbacks: usize,
    provider_observation: [u8; 32],
    collected_registration: bool,
    #[serde(deserialize_with = "required_option")]
    native: Option<NativeObservation>,
}
fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: Request,
    sessions: [SessionObservation; 2],
    hostile_checks: usize,
    preserved_a: bool,
    restored_floors: bool,
}
impl Report {
    fn check(&self, request: &Request) -> Result<(), String> {
        request.check()?;
        if &self.request != request
            || self.hostile_checks != if request.mode.native() { 6 } else { 0 }
            || self.preserved_a != request.mode.native()
            || !self.restored_floors
        {
            return Err("exact pair report and custody completion".into());
        }
        for (row, record) in self.sessions.iter().zip(&request.contexts) {
            if row.context != record.identity()
                || row.configs != 1
                || row.callbacks != 1
                || !row.collected_registration
                || row.provider_observation != record.observation()
                || row.native.is_some() != request.mode.native()
            {
                return Err("exact genuine session completion".into());
            }
            if let Some(native) = &row.native {
                native.simulation.check()?;
                let identities = &native.identity;
                if [
                    identities.source,
                    identities.semantic,
                    identities.preflight,
                    identities.ranked,
                    identities.execution,
                    identities.original,
                    identities.historical,
                    identities.promoted,
                    identities.input,
                    identities.output,
                    identities.llvm,
                ]
                .contains(&[0; 32])
                    || native.identity.source != native.identity.preflight
                    || native.identity.helpers
                        != if record.case().unit_local() {
                            [1, 1]
                        } else {
                            [0, 0]
                        }
                    || if record.case().motion() {
                        native.identity.hoists < 2
                            || native.identity.input == native.identity.output
                    } else {
                        native.identity.hoists != 0
                            || native.identity.input != native.identity.output
                    }
                {
                    return Err("actual source motion/helper/native identity".into());
                }
            }
        }
        if request.mode.native() {
            let a = &self.sessions[0].native.as_ref().unwrap().identity;
            let b = &self.sessions[1].native.as_ref().unwrap().identity;
            if a.source == b.source
                || a.semantic == b.semantic
                || a.execution == b.execution
                || a.ranked == b.ranked
            {
                return Err("actual unequal retained subjects required".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ConfigFailure(String);
struct SessionCallbacks<F, R> {
    context: Context,
    native: bool,
    configs: usize,
    calls: usize,
    next: F,
    result: Option<Result<(SessionObservation, Option<R>), String>>,
}
impl<F, R> Callbacks for SessionCallbacks<F, R>
where
    F: for<'tcx> FnMut(
            crate::production_pipeline::ProductionCompilation<
                'tcx,
                crate::production_pipeline::CollectedRustStage<'tcx>,
            >,
        ) -> Result<R, String>
        + Send,
    R: Send,
{
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        self.configs += 1;
        if self.configs != 1 {
            panic_any(ConfigFailure("exactly one config callback".into()));
        }
        if let Err(error) = self.context.install(config) {
            panic_any(ConfigFailure(error));
        }
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            if self.configs != 1 || self.calls != 1 {
                return Err("exact context callback roster".into());
            }
            let provider_observation = self.context.observe(tcx)?;
            let collected = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let imported = if self.native {
                Some((self.next)(collected)?)
            } else {
                drop(collected);
                None
            };
            Ok((
                SessionObservation {
                    context: self.context.record().identity(),
                    configs: self.configs,
                    callbacks: self.calls,
                    provider_observation,
                    collected_registration: true,
                    native: None,
                },
                imported,
            ))
        })());
        Compilation::Stop
    }
}
fn run_session<F, R>(
    context: Context,
    args: &[String],
    native: bool,
    next: F,
) -> Result<(SessionObservation, Option<R>), String>
where
    F: for<'tcx> FnMut(
            crate::production_pipeline::ProductionCompilation<
                'tcx,
                crate::production_pipeline::CollectedRustStage<'tcx>,
            >,
        ) -> Result<R, String>
        + Send,
    R: Send,
{
    fn is_send<T: Send>(_: &T) {}
    let mut callback = SessionCallbacks {
        context,
        native,
        configs: 0,
        calls: 0,
        next,
        result: None,
    };
    is_send(&callback);
    let result = catch_unwind(AssertUnwindSafe(|| {
        rustc_driver::run_compiler(args, &mut callback)
    }));
    if let Err(payload) = result {
        match payload.downcast::<ConfigFailure>() {
            Ok(error) => return Err(error.0),
            Err(payload) => resume_unwind(payload),
        }
    }
    callback.context.check_environment()?;
    if callback.configs != 1 || callback.calls != 1 {
        return Err("two sessions each require one Config and one analysis callback".into());
    }
    callback
        .result
        .ok_or("missing actual session observation")?
}

fn reserve_pair(
    a: &mut Budget<'_>,
    b: &mut Budget<'_>,
    incoming: [usize; 2],
) -> Result<(), Resource> {
    a.reserve_storage(incoming[0])?;
    if let Err(error) = b.reserve_storage(incoming[1]) {
        a.release_storage(incoming[0])?;
        return Err(error);
    }
    Ok(())
}
#[derive(Debug)]
struct SwapPanic;

fn execute(request: &Request, args: &[Vec<String>; 2]) -> Result<Report, String> {
    request.check()?;
    let mut slot =
        crate::production_pipeline::RankedVerifiedProductionCompilation::source_test_pair_slot_v1();
    let mut aw = Work::new(work_limit());
    let mut bw = Work::new(work_limit());
    let mut a = Budget::new(&mut aw, storage_limit());
    let mut b = Budget::new(&mut bw, storage_limit());
    let siblings = [vec![0x31u8; 31], vec![0x47u8; 47]];
    let sibling_bytes = siblings
        .each_ref()
        .map(|v| std::mem::size_of_val(v) + v.capacity());
    a.reserve_storage(37 + sibling_bytes[0]).unwrap();
    b.reserve_storage(41 + sibling_bytes[1]).unwrap();
    a.charge_work(7).unwrap();
    b.charge_work(11).unwrap();
    let floors = [a.storage(), b.storage()];
    let ledgers = [a.work_ledger_identity_v1(), b.work_ledger_identity_v1()];
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<Report, String> {
        let mut rows = Vec::new();
        let mut a_before_b = None;
        for index in 0..2 {
            let context = Context::from_record(request.contexts[index].clone(), &args[index])?;
            let (mut row, imported) =
                run_session(context, &args[index], request.mode.native(), |collected| {
                    collected
                        .source_test_import_stage_v1()
                        .map_err(|e| format!("genuine paired source import prerequisite: {e:?}"))
                })?;
            if row.native.is_some() || imported.is_some() != request.mode.native() {
                return Err("exact imported-stage handoff before native observation".into());
            }
            if let Some(imported) = imported {
                let ranked = imported
                    .into_ranked_v1()
                    .map_err(|e| format!("genuine paired source/ranked prerequisite: {e:?}"))?;
                let record = &request.contexts[index];
                let expected = if record.case().unit_local() {
                    fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::UnitLocal
                } else {
                    fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
                };
                if ranked.checked_output_source_policy_v1() != expected {
                    return Err("actual paired source route".into());
                }
                let budget = if index == 0 { &mut a } else { &mut b };
                row.native = Some({
                    slot.capture_v1(index == 1, ranked, budget)
                        .map_err(|e| format!("genuine paired native prerequisite: {e:?}"))?;
                    slot.with_owner_v1(index == 1, |owner| -> Result<NativeObservation, String> {
                        owner
                            .verify_equivalence(budget)
                            .map_err(|e| format!("actual paired native replay: {e:?}"))?;
                        Ok(owner.with_source_test_observation_v1(|view| {
                            let input = sim::Input {
                                original: view.original().unwrap(),
                                historical: view.historical_p8(),
                                promoted: view.promoted(),
                                input: view.input(),
                                output: view.output(),
                                origins: view.origins(),
                                launch: view.source_launch(),
                                semantic: view.source_semantic(),
                                kernels: view.kernels(),
                                profile: view.profile(),
                            };
                            assert_eq!(view.unit_local(), record.case().unit_local());
                            assert_eq!(view.profile(), profile(record.target()).unwrap());
                            assert!(!owner.grants_artifact_or_launch_authority());
                            NativeObservation {
                                identity: identity(
                                    &input,
                                    record.case(),
                                    (
                                        view.source_identity(),
                                        view.preflight_identity(),
                                        view.ranked_identity(),
                                    ),
                                    view.execution_bytes(),
                                    owner.llvm_ir(),
                                    view.deleted_helpers(),
                                ),
                                simulation: sim::observe(&input, record.case()),
                            }
                        }))
                    })?
                });
            }
            rows.push(row);
            let state = (a.storage(), a.work(), a.peak_storage(), a.failed_storage());
            if index == 0 {
                a_before_b = Some(state);
            } else {
                assert_eq!(
                    Some(state),
                    a_before_b,
                    "B session cannot alter A's original meter or floor"
                );
            }
        }
        assert!(a.work_ledger_identity_v1() == ledgers[0]);
        assert!(b.work_ledger_identity_v1() == ledgers[1]);
        let mut checks = 0;
        if request.mode.native() {
            let before = &rows[0].native.as_ref().unwrap().identity;
            let after_a = slot.with_owner_v1(false, |owner| {
                owner.verify_equivalence(&mut a).unwrap();
                owner.with_source_test_observation_v1(|view| {
                    let input = sim::Input {
                        original: view.original().unwrap(),
                        historical: view.historical_p8(),
                        promoted: view.promoted(),
                        input: view.input(),
                        output: view.output(),
                        origins: view.origins(),
                        launch: view.source_launch(),
                        semantic: view.source_semantic(),
                        kernels: view.kernels(),
                        profile: view.profile(),
                    };
                    identity(
                        &input,
                        request.contexts[0].case(),
                        (
                            view.source_identity(),
                            view.preflight_identity(),
                            view.ranked_identity(),
                        ),
                        view.execution_bytes(),
                        owner.llvm_ir(),
                        view.deleted_helpers(),
                    )
                })
            });
            assert_eq!(
                before, &after_a,
                "A retained on original ledger across B session"
            );
            let right = &rows[1].native.as_ref().unwrap().identity;
            assert_ne!(before.source, right.source);
            assert_ne!(before.semantic, right.semantic);
            assert_ne!(before.execution, right.execution);
            assert_ne!(before.ranked, right.ranked);
            let extents = slot.field_extents_v1();
            let unequal_extent = slot.with_both_v1(|left, right| {
                left.with_source_test_observation_v1(|l| {
                    right.with_source_test_observation_v1(|r| {
                        l.execution_bytes().len() != r.execution_bytes().len()
                    })
                })
            });
            for plan in [false, true] {
                let incoming = [extents[1][usize::from(plan)], extents[0][usize::from(plan)]];
                let held = [a.storage(), b.storage()];
                let accepted = [a.work(), b.work()];
                let peaks = [a.peak_storage(), b.peak_storage()];
                let history = [a.failed_storage(), b.failed_storage()];
                let short = incoming[1].checked_sub(1).unwrap();
                let denied_total = b.storage_limit().checked_add(1).unwrap();
                let padding = b
                    .storage_limit()
                    .checked_sub(b.storage())
                    .and_then(|n| n.checked_sub(short))
                    .expect("actual-owner second-denial capacity prerequisite");
                // Conservative temporary ledger prepayment, not a claim of extra heap allocation.
                b.reserve_storage(padding).unwrap();
                let denied = catch_unwind(AssertUnwindSafe(|| {
                    let error = reserve_pair(&mut a, &mut b, incoming).unwrap_err();
                    assert!(
                        matches!(error,Resource::Storage(error) if error.actual()==denied_total && error.limit()==b.storage_limit())
                    );
                    assert_eq!(
                        (a.storage(), b.storage()),
                        (held[0], held[1].checked_add(padding).unwrap())
                    );
                    assert_eq!((a.work(), b.work()), (accepted[0], accepted[1]));
                    assert_eq!(
                        (a.peak_storage(), b.peak_storage()),
                        (
                            peaks[0].max(held[0].checked_add(incoming[0]).unwrap()),
                            peaks[1].max(held[1].checked_add(padding).unwrap())
                        )
                    );
                    assert_eq!(a.failed_storage(), history[0]);
                    assert_eq!(b.failed_storage(), history[1].or(Some(denied_total)));
                    assert!(a.work_ledger_identity_v1() == ledgers[0]);
                    assert!(b.work_ledger_identity_v1() == ledgers[1]);
                }));
                b.release_storage(padding).unwrap();
                if let Err(payload) = denied {
                    resume_unwind(payload);
                }
                checks += 1;
                slot.with_both_v1(|left, right| {
                    left.verify_equivalence(&mut a).unwrap();
                    right.verify_equivalence(&mut b).unwrap();
                });
                for panics in [false, true] {
                    reserve_pair(&mut a, &mut b, incoming).unwrap();
                    let swapped = catch_unwind(AssertUnwindSafe(|| {
                        slot.with_swapped_v1(plan, |left, right| {
                            if panics {
                                panic_any(SwapPanic);
                            }
                            left.verify_equivalence(&mut a)
                                .unwrap_err()
                                .source_test_assert_pair_refusal_v1(plan, unequal_extent);
                            right
                                .verify_equivalence(&mut b)
                                .unwrap_err()
                                .source_test_assert_pair_refusal_v1(plan, unequal_extent);
                        })
                    }));
                    b.release_storage(incoming[1]).unwrap();
                    a.release_storage(incoming[0]).unwrap();
                    assert_eq!((a.storage(), b.storage()), (held[0], held[1]));
                    assert!(a.work_ledger_identity_v1() == ledgers[0]);
                    assert!(b.work_ledger_identity_v1() == ledgers[1]);
                    match swapped {
                        Ok(()) => assert!(!panics),
                        Err(payload) => {
                            if !panics || !payload.is::<SwapPanic>() {
                                resume_unwind(payload);
                            }
                        }
                    }
                    slot.with_both_v1(|left, right| {
                        left.verify_equivalence(&mut a).unwrap();
                        right.verify_equivalence(&mut b).unwrap();
                    });
                    checks += 1;
                }
            }
        }
        Ok(Report {
            request: request.clone(),
            sessions: rows.try_into().unwrap(),
            hostile_checks: checks,
            preserved_a: request.mode.native(),
            restored_floors: true,
        })
    }));
    slot.finish_v1(&mut a, &mut b)
        .map_err(|e| format!("actual paired owner destruction/accounting: {e:?}"))?;
    assert_eq!([a.storage(), b.storage()], floors);
    assert!(a.work_ledger_identity_v1() == ledgers[0]);
    assert!(b.work_ledger_identity_v1() == ledgers[1]);
    assert!(siblings[0].iter().all(|v| *v == 0x31));
    assert!(siblings[1].iter().all(|v| *v == 0x47));
    drop(siblings);
    a.release_storage(sibling_bytes[0]).unwrap();
    b.release_storage(sibling_bytes[1]).unwrap();
    assert_eq!((a.storage(), b.storage()), (37, 41));
    a.release_storage(37).unwrap();
    b.release_storage(41).unwrap();
    let report = match result {
        Ok(value) => value?,
        Err(payload) => resume_unwind(payload),
    };
    report.check(request)?;
    Ok(report)
}

#[test]
#[ignore = "parent-only helper: exact captured two-session contexts and actual source prerequisites"]
fn licm_native_session_pair_child() {
    let request: Request = serde_json::from_str(&env::var(PAIR_REQUEST).unwrap()).unwrap();
    let args: [Vec<String>; 2] =
        serde_json::from_slice(&std::fs::read(env::var_os(PAIR_ARGS).unwrap()).unwrap()).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| execute(&request, &args))).unwrap_or_else(|_| {
        Err("genuine two-session context/source/native prerequisite panicked".into())
    });
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(env::var_os(PAIR_RESULT).unwrap())
        .unwrap()
        .write_all(&serde_json::to_vec(&result).unwrap())
        .unwrap();
    assert!(
        result.is_ok(),
        "two genuine sequential sessions: {result:?}"
    );
}

fn roster(native: bool) -> Vec<(String, Mode)> {
    ["gfx942", "gfx950"]
        .into_iter()
        .flat_map(|target| {
            let modes = if native {
                vec![Mode::NativeDirect, Mode::NativeUnitLocal]
            } else {
                vec![Mode::ContextOnly]
            };
            modes.into_iter().map(move |mode| (target.to_owned(), mode))
        })
        .collect()
}
fn emit_observation(
    writer: &mut impl std::io::Write,
    request: &Request,
    report: &Report,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<[u8; 32], String> {
    report.check(request)?;
    let observation=serde_json::to_vec(&serde_json::json!({"child_test":PAIR_CHILD,"request":request,"report":report,
        "stdout_bytes":stdout.len(),"stdout_sha256":digest(stdout),"stderr_bytes":stderr.len(),"stderr_sha256":digest(stderr),"qualification":false})).map_err(|e|e.to_string())?;
    for bytes in [
        b"FE2O3_LICM_SESSION_PAIR_OBSERVATION ".as_slice(),
        &observation,
        b"\nPAIR_STDOUT_BEGIN\n",
        stdout,
        b"\nPAIR_STDOUT_END\nPAIR_STDERR_BEGIN\n",
        stderr,
        b"\nPAIR_STDERR_END\n",
    ] {
        writer.write_all(bytes).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())?;
    Ok(digest(&observation))
}
type Completion = (usize, String, Mode, [[u8; 32]; 2], [u8; 32]);
fn emit_completion(
    writer: &mut impl std::io::Write,
    native: bool,
    completed: &[Completion],
) -> Result<(), String> {
    let expected = roster(native);
    if completed.len() != expected.len() {
        return Err("complete exact pair roster".into());
    }
    for (index, (row, (target, mode))) in completed.iter().zip(expected).enumerate() {
        if row.0 != index + 1
            || row.1 != target
            || row.2 != mode
            || row.3[0] == row.3[1]
            || row.3.contains(&[0; 32])
            || row.4 == [0; 32]
        {
            return Err("ordered pair completion identities".into());
        }
    }
    writeln!(writer,"FE2O3_LICM_SESSION_PAIR_COMPLETED {}",serde_json::to_string(&serde_json::json!({"pairs":completed,"compiler_callbacks":if native{8}else{4},"child_harnesses":if native{4}else{2},"qualification":false})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
fn parent(native: bool) {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-immutable-session-pairs");
    let root = workspace();
    let mut completed = Vec::new();
    let stderr = std::io::stderr();
    let mut log = stderr.lock();
    for (index, (target, mode)) in roster(native).into_iter().enumerate() {
        let directory = scratch.path().join(index.to_string());
        std::fs::create_dir(&directory).unwrap();
        let mut captures = Vec::new();
        for (position, case) in mode.cases().into_iter().enumerate() {
            let case_directory = directory.join(position.to_string());
            std::fs::create_dir(&case_directory).unwrap();
            let mut captured = corpus_cargo::capture(
                &root,
                &fixture(case, &target),
                &case_directory,
                &scratch.path().join(&target),
            )
            .unwrap();
            captured.args.push("-Zmir-opt-level=0".into());
            captured.args.push("-Zinline-mir=no".into());
            captures.push(captured);
        }
        let contexts = std::array::from_fn(|i| {
            Context::capture(
                &captures[i],
                &captures[0].environment,
                mode.cases()[i],
                &target,
            )
            .unwrap()
        });
        let request = Request {
            ordinal: index + 1,
            mode,
            contexts,
        };
        request.check().unwrap();
        assert_eq!(captures[0].cwd, captures[1].cwd);
        let args_path = directory.join("args.json");
        let result_path = directory.join("result.json");
        std::fs::write(
            &args_path,
            serde_json::to_vec(&[&captures[0].args, &captures[1].args]).unwrap(),
        )
        .unwrap();
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .env_clear()
            .envs(captures[0].environment.iter().cloned())
            .current_dir(&captures[0].cwd)
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove(CHILD_PROOF_PROBE)
            .env(PAIR_REQUEST, serde_json::to_string(&request).unwrap())
            .env(PAIR_ARGS, &args_path)
            .env(PAIR_RESULT, &result_path)
            .args(["--exact", PAIR_CHILD, "--ignored", "--nocapture"]);
        progress::clear_inherited_jobserver(&mut command);
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "actual paired child: {}",
            corpus_cargo::diagnostics(&result)
        );
        let stdout = std::str::from_utf8(&result.stdout).unwrap();
        assert_eq!(
            stdout
                .matches("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;")
                .count(),
            1
        );
        assert_eq!(
            stdout.matches(&format!("test {PAIR_CHILD} ... ok")).count(),
            1
        );
        let report: Result<Report, String> =
            serde_json::from_slice(&std::fs::read(&result_path).unwrap()).unwrap();
        let report = report.unwrap();
        report.check(&request).unwrap();
        for (i, capture) in captures.iter().enumerate() {
            assert_eq!(
                Context::capture(capture, &captures[0].environment, mode.cases()[i], &target)
                    .unwrap(),
                request.contexts[i]
            );
        }
        let observation =
            emit_observation(&mut log, &request, &report, &result.stdout, &result.stderr).unwrap();
        completed.push((
            index + 1,
            target,
            mode,
            request.contexts.map(|r| r.identity()),
            observation,
        ));
    }
    emit_completion(&mut log, native, &completed).unwrap();
}

#[test]
#[ignore = "requires genuine target Cargo captures; context/collected registration only, no protected/native qualification"]
fn ordinary_rust_immutable_context_two_profiles_without_native_admission() {
    parent(false);
}
#[test]
#[ignore = "requires genuine unequal admitted source/native owners and protected runtime; missing runtime is hard failure"]
fn ordinary_rust_native_licm_unequal_owner_pairs_two_routes_two_profiles() {
    parent(true);
}

#[test]
fn immutable_pair_rosters_count_children_separately_from_callbacks() {
    assert_eq!(
        roster(false),
        vec![
            ("gfx942".into(), Mode::ContextOnly),
            ("gfx950".into(), Mode::ContextOnly)
        ]
    );
    assert_eq!(
        roster(true),
        vec![
            ("gfx942".into(), Mode::NativeDirect),
            ("gfx942".into(), Mode::NativeUnitLocal),
            ("gfx950".into(), Mode::NativeDirect),
            ("gfx950".into(), Mode::NativeUnitLocal)
        ]
    );
    assert_eq!(
        Mode::ContextOnly.cases(),
        [Case::DirectNoop, Case::UnitLocalNoop]
    );
    assert_eq!(
        Mode::NativeDirect.cases(),
        [Case::DirectMotion, Case::DirectNoop]
    );
    assert_eq!(
        Mode::NativeUnitLocal.cases(),
        [Case::UnitLocalMotion, Case::UnitLocalNoop]
    );
}
#[test]
fn immutable_pair_second_reservation_denial_rolls_back_only_first_temporary_charge() {
    let mut aw = Work::new(100);
    let mut bw = Work::new(100);
    let mut a = Budget::new(&mut aw, 100);
    let mut b = Budget::new(&mut bw, 20);
    a.reserve_storage(7).unwrap();
    b.reserve_storage(11).unwrap();
    a.charge_work(3).unwrap();
    b.charge_work(5).unwrap();
    let error = reserve_pair(&mut a, &mut b, [13, 10]).unwrap_err();
    assert!(matches!(error,Resource::Storage(e) if e.actual()==21 && e.limit()==20));
    assert_eq!(
        (
            a.storage(),
            b.storage(),
            a.work(),
            b.work(),
            a.peak_storage(),
            b.peak_storage()
        ),
        (7, 11, 3, 5, 20, 11)
    );
    assert_eq!(a.failed_storage(), None);
    assert_eq!(b.failed_storage(), Some(21));
    assert!(
        matches!(reserve_pair(&mut a,&mut b,[13,11]),Err(Resource::Storage(e)) if e.actual()==22 && e.limit()==20)
    );
    assert_eq!(b.failed_storage(), Some(21));
    assert!(
        matches!(reserve_pair(&mut a,&mut b,[94,1]),Err(Resource::Storage(e)) if e.actual()==101 && e.limit()==100)
    );
    assert_eq!(a.failed_storage(), Some(101));
    assert_eq!(b.failed_storage(), Some(21));
    assert_eq!(
        (
            a.storage(),
            b.storage(),
            a.work(),
            b.work(),
            a.peak_storage(),
            b.peak_storage()
        ),
        (7, 11, 3, 5, 20, 11)
    );
}

fn inert_context_report() -> Report {
    let request = Request {
        ordinal: 1,
        mode: Mode::ContextOnly,
        contexts: [
            context::tests::inert_for_case(Case::DirectNoop, "gfx942", "a"),
            context::tests::inert_for_case(Case::UnitLocalNoop, "gfx942", "b"),
        ],
    };
    Report {
        sessions: request
            .contexts
            .each_ref()
            .map(|record| SessionObservation {
                context: record.identity(),
                configs: 1,
                callbacks: 1,
                provider_observation: record.observation(),
                collected_registration: true,
                native: None,
            }),
        request,
        hostile_checks: 0,
        preserved_a: false,
        restored_floors: true,
    }
}
#[test]
fn immutable_pair_protocol_rejects_missing_duplicate_reordered_and_unequal_subject_substitution() {
    let report = inert_context_report();
    report.check(&report.request).unwrap();
    let json = serde_json::to_value(&report).unwrap();
    for name in json.as_object().unwrap().keys() {
        let mut changed = json.clone();
        changed.as_object_mut().unwrap().remove(name);
        assert!(serde_json::from_value::<Report>(changed).is_err());
    }
    for name in json["sessions"][0].as_object().unwrap().keys() {
        let mut changed = json.clone();
        changed["sessions"][0].as_object_mut().unwrap().remove(name);
        assert!(serde_json::from_value::<Report>(changed).is_err());
    }
    for name in json["request"].as_object().unwrap().keys() {
        let mut changed = json.clone();
        changed["request"].as_object_mut().unwrap().remove(name);
        assert!(serde_json::from_value::<Report>(changed).is_err());
    }
    let text = serde_json::to_string(&report).unwrap().replacen(
        "\"configs\":1",
        "\"configs\":1,\"configs\":1",
        1,
    );
    assert!(serde_json::from_str::<Report>(&text).is_err());
    let mut unknown = json.clone();
    unknown["sessions"][0]["extra"] = true.into();
    assert!(serde_json::from_value::<Report>(unknown).is_err());
    for index in 0..9 {
        let mut changed = report.clone();
        match index {
            0 => changed.sessions.swap(0, 1),
            1 => changed.sessions[1].callbacks = 0,
            2 => changed.sessions[0].configs = 2,
            3 => {
                changed.sessions[0].provider_observation = changed.sessions[1].provider_observation
            }
            4 => changed.sessions[0].collected_registration = false,
            5 => changed.hostile_checks = 1,
            6 => changed.preserved_a = true,
            7 => changed.restored_floors = false,
            _ => changed.request.ordinal += 1,
        };
        assert!(changed.check(&report.request).is_err());
    }
    let mut equal = report.request.clone();
    equal.contexts[1] = equal.contexts[0].clone();
    assert!(equal.check().is_err());
    let mut wrong = report.request.clone();
    wrong.contexts.swap(0, 1);
    assert!(wrong.check().is_err());
    let mut native = report.request.clone();
    native.mode = Mode::NativeDirect;
    assert!(native.check().is_err());
}
#[test]
fn immutable_pair_observation_preserves_raw_bytes_and_closed_completion_roster() {
    // Inert format-only records are not actual source/runtime observations.
    let report = inert_context_report();
    let stdout = b"\xffstdout\0";
    let stderr = b"\x80stderr\n";
    let mut bytes = Vec::new();
    let hash = emit_observation(&mut bytes, &report.request, &report, stdout, stderr).unwrap();
    let newline = bytes.iter().position(|b| *b == b'\n').unwrap();
    let prefix = b"FE2O3_LICM_SESSION_PAIR_OBSERVATION ";
    assert!(bytes.starts_with(prefix));
    let json = &bytes[prefix.len()..newline];
    assert_eq!(hash, digest(json));
    let decoded: serde_json::Value = serde_json::from_slice(json).unwrap();
    assert_eq!(decoded["qualification"], false);
    assert_eq!(decoded["stdout_bytes"], stdout.len());
    assert_eq!(decoded["stderr_bytes"], stderr.len());
    assert_eq!(
        decoded["stdout_sha256"],
        serde_json::to_value(digest(stdout)).unwrap()
    );
    assert_eq!(
        decoded["stderr_sha256"],
        serde_json::to_value(digest(stderr)).unwrap()
    );
    let mut expected = b"\nPAIR_STDOUT_BEGIN\n".to_vec();
    expected.extend_from_slice(stdout);
    expected.extend_from_slice(b"\nPAIR_STDOUT_END\nPAIR_STDERR_BEGIN\n");
    expected.extend_from_slice(stderr);
    expected.extend_from_slice(b"\nPAIR_STDERR_END\n");
    assert_eq!(&bytes[newline..], expected);
    let mut invalid = report.clone();
    invalid.sessions[1].callbacks = 0;
    let mut empty = Vec::new();
    assert!(emit_observation(&mut empty, &report.request, &invalid, stdout, stderr).is_err());
    assert!(empty.is_empty());
    let mut short = [0u8; 9];
    assert!(
        emit_observation(
            &mut short.as_mut_slice(),
            &report.request,
            &report,
            stdout,
            stderr
        )
        .is_err()
    );
    let complete = vec![
        (
            1,
            "gfx942".into(),
            Mode::ContextOnly,
            [[1; 32], [2; 32]],
            hash,
        ),
        (
            2,
            "gfx950".into(),
            Mode::ContextOnly,
            [[3; 32], [4; 32]],
            hash,
        ),
    ];
    let mut framed = Vec::new();
    emit_completion(&mut framed, false, &complete).unwrap();
    let completion: serde_json::Value =
        serde_json::from_slice(&framed[b"FE2O3_LICM_SESSION_PAIR_COMPLETED ".len()..]).unwrap();
    assert_eq!(completion["compiler_callbacks"], 4);
    assert_eq!(completion["child_harnesses"], 2);
    assert_eq!(completion["qualification"], false);
    assert!(emit_completion(&mut Vec::new(), false, &complete[..1]).is_err());
    let mut wrong = complete.clone();
    wrong.swap(0, 1);
    assert!(emit_completion(&mut Vec::new(), false, &wrong).is_err());
    wrong = complete.clone();
    wrong[1] = wrong[0].clone();
    assert!(emit_completion(&mut Vec::new(), false, &wrong).is_err());
    wrong = complete;
    wrong[1].3[1] = wrong[1].3[0];
    assert!(emit_completion(&mut Vec::new(), false, &wrong).is_err());
}
