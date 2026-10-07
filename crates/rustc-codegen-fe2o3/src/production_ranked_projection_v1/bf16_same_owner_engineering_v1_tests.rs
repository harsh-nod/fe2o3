//! Inert budget/control tests and genuine-owner refusal probes.
//! These controls never substitute an engine success for actual source evidence.
use super::*;
use fe2o3_compiler_ffi::{CompilerModuleHandoffErrorV1, CompilerModuleHandoffErrorV2};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
pub(crate) struct Snapshot {
    pub(crate) engine_entries: usize,
    pub(crate) collected_rows: usize,
    pub(crate) postflights: usize,
    pub(crate) row: Option<Row>,
}
#[derive(Clone, Copy)]
struct State {
    deny_entry: bool,
    snapshot: Snapshot,
    events: [u8; 8],
    event_count: usize,
}
std::thread_local! {
    static STATE: Cell<Option<State>> = const { Cell::new(None) };
}
pub(crate) struct ObservationScope {
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl ObservationScope {
    fn begin(deny_entry: bool) -> Result<Self, PrivateBf16LlvmErrorV1> {
        STATE.with(|cell| {
            if cell.get().is_some() {
                return Err(mismatch("nested same-owner engineering observation"));
            }
            cell.set(Some(State {
                deny_entry,
                snapshot: Snapshot::default(),
                events: [0; 8],
                event_count: 0,
            }));
            Ok(Self {
                _thread_bound: std::marker::PhantomData,
            })
        })
    }
    pub(crate) fn snapshot(&self) -> Snapshot {
        STATE.with(|cell| cell.get().expect("active same-owner observation").snapshot)
    }
    fn events(&self) -> ([u8; 8], usize) {
        STATE.with(|cell| {
            let state = cell.get().expect("active event scope");
            (state.events, state.event_count)
        })
    }
    fn entries(&self) -> Option<usize> {
        STATE.with(|cell| cell.get().map(|s| s.snapshot.engine_entries))
    }
}
impl Drop for ObservationScope {
    fn drop(&mut self) {
        STATE.with(|cell| cell.set(None));
    }
}
pub(super) fn before_engine() -> Result<(), Failure> {
    STATE.with(|cell| match cell.get() {
        None => Ok(()),
        Some(mut state) => {
            state.snapshot.engine_entries = state.snapshot.engine_entries.checked_add(1).unwrap();
            cell.set(Some(state));
            if state.deny_entry {
                Err(Failure::EntryGuard)
            } else {
                Ok(())
            }
        }
    })
}
pub(super) fn record_row(row: Row) {
    STATE.with(|cell| {
        if let Some(mut state) = cell.get() {
            state.snapshot.collected_rows = state.snapshot.collected_rows.checked_add(1).unwrap();
            assert!(
                state.snapshot.row.is_none(),
                "one fixed engine row per observation"
            );
            state.snapshot.row = Some(row);
            cell.set(Some(state));
        }
    });
}
pub(super) fn record_postflight() {
    STATE.with(|cell| {
        if let Some(mut state) = cell.get() {
            state.snapshot.postflights = state.snapshot.postflights.checked_add(1).unwrap();
            cell.set(Some(state));
        }
    });
}
impl PrivateBf16WorkerHandoffV1 {
    /// Only fixed harness state; no source/engine owner, image or authority.
    pub(crate) fn begin_same_owner_engineering_observation_for_test_v1()
    -> Result<ObservationScope, PrivateBf16LlvmErrorV1> {
        ObservationScope::begin(false)
    }
}

// Fixed test-only event storage, explicitly outside production logical custody.
fn event(value: u8) {
    STATE.with(|cell| {
        if let Some(mut state) = cell.get() {
            assert!(state.event_count < state.events.len());
            state.events[state.event_count] = value;
            state.event_count += 1;
            cell.set(Some(state));
        }
    });
}
pub(super) fn before_refund() {
    event(2);
}
struct DropEvent(u8);
impl Drop for DropEvent {
    fn drop(&mut self) {
        event(self.0);
    }
}

// Original-owner negative controls, not a detached admission constructor. The
// existing public-codec/engine allocations are not newly brought into Budget.
impl PrivateBf16WorkerHandoffV1 {
    fn same_owner_engineering_checkpoint_for_test_v1(
        &mut self,
    ) -> Result<
        (
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            usize,
        ),
        PrivateBf16LlvmErrorV1,
    > {
        self.descriptor
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| {
                budget
                    .check_prior_denials_v1()
                    .map_err(E::ConditionalResource)?;
                Ok((
                    budget.work_ledger_identity_v1(),
                    budget.storage(),
                    budget.work(),
                    budget.peak_storage(),
                ))
            })
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)
    }

    #[allow(dead_code)]
    pub(crate) fn exercise_same_owner_engineering_refusals_for_test_v1(
        &mut self,
        requested: [u8; 4],
        roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        worker: &PinnedWorkerV1,
        output_bytes: u64,
        limits: WorkerExecutionLimitsV1,
        deadline: Instant,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        if profile != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942 {
            return Err(mismatch("entry controls require the genuine Gfx942 owner"));
        }
        let scope = ObservationScope::begin(true)?;
        self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile)?;
        let wrong = if requested == [0, 1, 2, 3] {
            [1, 0, 2, 3]
        } else {
            [0, 1, 2, 3]
        };
        let descriptor_last = self
            .descriptor
            .descriptor
            .canonical_descriptor
            .len()
            .checked_sub(1)
            .ok_or_else(|| mismatch("empty genuine descriptor"))?;
        let text_at = self
            .descriptor
            .descriptor
            .final_llvm
            .find("gfx942")
            .ok_or_else(|| mismatch("genuine final target absent"))?;
        let handoff_last = self
            .handoff
            .canonical
            .len()
            .checked_sub(1)
            .ok_or_else(|| mismatch("empty genuine handoff"))?;
        for fault in 0..6 {
            let before = self.same_owner_engineering_checkpoint_for_test_v1()?;
            let output =
                WorkerOutputConstraintsV1::new(output_bytes).map_err(|_| mismatch(LIMIT))?;
            if fault == 3 {
                self.descriptor.descriptor.canonical_descriptor[descriptor_last] ^= 1;
            }
            if fault == 4 {
                self.descriptor
                    .descriptor
                    .final_llvm
                    .get_mut(text_at..text_at + 6)
                    .unwrap()
                    .make_ascii_uppercase();
            }
            if fault == 5 {
                self.handoff.canonical[handoff_last] ^= 1;
            }
            let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.observe_same_owner_engineering_v1(
                    if fault == 0 { wrong } else { requested },
                    if fault == 2 { &[] } else { roots },
                    if fault == 1 {
                        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950
                    } else {
                        profile
                    },
                    worker,
                    output,
                    limits,
                    deadline,
                )
            }));
            // Restore the same retained allocations before inspecting either the
            // Result or an unwind payload. No replacement box/source owner.
            if fault == 3 {
                self.descriptor.descriptor.canonical_descriptor[descriptor_last] ^= 1;
            }
            if fault == 4 {
                self.descriptor
                    .descriptor
                    .final_llvm
                    .get_mut(text_at..text_at + 6)
                    .unwrap()
                    .make_ascii_lowercase();
            }
            if fault == 5 {
                self.handoff.canonical[handoff_last] ^= 1;
            }
            let result = match attempted {
                Ok(value) => value,
                Err(payload) => {
                    drop(payload);
                    Err(mismatch(
                        "private Worker negative control unwound after restoration",
                    ))
                }
            };
            if scope.entries() != Some(0) {
                drop(result);
                return Err(mismatch(ENTRY_GUARD));
            }
            // Sticky denial wins and unknown credit is never released here.
            let after = self.same_owner_engineering_checkpoint_for_test_v1()?;
            if before.0 != after.0
                || before.1 != after.1
                || before.2 >= after.2
                || before.3 > after.3
            {
                return Err(private_bf16_target_resource_v1(Resource::Accounting));
            }
            let exact = match (&result, fault) {
                (Err(PrivateBf16LlvmErrorV1::RankedVerification(E::FormalMemory(
                    fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::SemanticKir(
                        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch)))), 0) => true,
                (Err(PrivateBf16LlvmErrorV1::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure)), 1 | 2) => true,
                (Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                    crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                        "private BF16 descriptor/final LLVM bytes differ from owned replay"))), 3 | 4) => true,
                (Err(PrivateBf16LlvmErrorV1::WorkerHandoff(HandoffError::Handoff(
                    CompilerModuleHandoffErrorV2::Handoff(CompilerModuleHandoffErrorV1::ModuleIdentityMismatch)))), 5) => true,
                _ => false,
            };
            if !exact {
                return match result {
                    Err(error) => Err(error),
                    Ok(()) => Err(mismatch(
                        "private Worker negative control unexpectedly succeeded",
                    )),
                };
            }
            self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile)?;
        }
        if scope.entries() != Some(0) {
            return Err(mismatch(ENTRY_GUARD));
        }
        drop(scope);
        Ok(())
    }
}

fn row() -> Row {
    let id = ContentIdentityV1::from_parts([7; 32], 19);
    Row {
        handoff: id,
        hsaco: id,
        worker: id,
        descriptor_sha: [8; 32],
        descriptor_bytes: 3,
        bootstrap_request: id,
        bootstrap_response: id,
        replay_request: id,
        replay_response: id,
    }
}
struct DropCount<'a>(&'a Cell<usize>);
impl Drop for DropCount<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn is_reason(result: &Result<(), PrivateBf16LlvmErrorV1>, reason: &str) -> bool {
    matches!(result, Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
        crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(r))) if *r == reason)
}
#[test]
fn closed_loan_success_drops_captures_and_restores_original_floor() {
    let dropped = Cell::new(0);
    let emit_count = Cell::new(0);
    let mut work = Work::new(1000);
    let mut b = Budget::new(&mut work, 4096);
    b.reserve_storage(7).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let capture = DropCount(&dropped);
    run_closed(
        &mut b,
        37,
        move || {
            drop(capture);
            Ok(row())
        },
        |_| {
            emit_count.set(1);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        (dropped.get(), emit_count.get(), b.storage(), b.work()),
        (1, 1, 7, 37)
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    assert!(b.peak_storage() > 7);
}
#[test]
fn closed_loan_refusal_drops_capture_without_emitting_or_refunding_work() {
    let dropped = Cell::new(0);
    let mut work = Work::new(1000);
    let mut b = Budget::new(&mut work, 4096);
    b.reserve_storage(9).unwrap();
    let capture = DropCount(&dropped);
    let got = run_closed(
        &mut b,
        41,
        move || {
            drop(capture);
            Err(Failure::Engine)
        },
        |_| panic!("must not emit"),
    );
    assert!(is_reason(&got, ENGINE));
    assert_eq!((dropped.get(), b.storage(), b.work()), (1, 9, 41));
}
#[test]
fn closed_loan_caught_unwind_drops_payload_and_capture_before_refund() {
    let dropped = Cell::new(0);
    let mut work = Work::new(1000);
    let mut b = Budget::new(&mut work, 4096);
    b.reserve_storage(11).unwrap();
    let capture = DropCount(&dropped);
    let got = run_closed(
        &mut b,
        43,
        move || {
            let _held = capture;
            std::panic::resume_unwind(Box::new("inert loan control"))
        },
        |_| Ok(()),
    );
    assert!(is_reason(&got, UNWIND));
    assert_eq!((dropped.get(), b.storage(), b.work()), (1, 11, 43));
}
#[test]
fn closed_loan_emitter_refusal_or_unwind_restores_known_floor() {
    for unwind in [false, true] {
        let mut work = Work::new(1000);
        let mut b = Budget::new(&mut work, 4096);
        b.reserve_storage(13).unwrap();
        let got = run_closed(
            &mut b,
            47,
            || Ok(row()),
            |_| {
                if unwind {
                    std::panic::resume_unwind(Box::new("inert emitter control"));
                }
                Err(Failure::Join)
            },
        );
        assert!(is_reason(&got, if unwind { UNWIND } else { JOIN }));
        assert_eq!(b.storage(), 13);
    }
}
#[test]
fn closed_loan_work_and_storage_denials_never_enter_callback() {
    for storage_denial in [false, true] {
        let called = Cell::new(false);
        let dropped = Cell::new(0);
        let capture = DropCount(&dropped);
        let mut work = Work::new(if storage_denial { 1000 } else { 0 });
        let mut b = Budget::new(&mut work, if storage_denial { 0 } else { 4096 });
        let got = run_closed(
            &mut b,
            1,
            || {
                let _held = capture;
                called.set(true);
                Ok(row())
            },
            |_| Ok(()),
        );
        assert!(matches!(
            got,
            Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(Resource::Storage(_) | Resource::Work(_))
            ))
        ));
        assert!(!called.get());
        assert_eq!(dropped.get(), 1);
        assert_eq!(b.storage(), 0);
        assert!(b.check_prior_denials_v1().is_err());
    }
}
#[test]
fn closed_loan_exact_and_one_short_selected_reservation() {
    let construct = || Ok(row());
    let emit = |_| Ok(());
    fn amount<F, G>(_: &F, _: &G) -> usize {
        scratch::<F, G>().unwrap()
    }
    let exact = amount(&construct, &emit);
    let mut w = Work::new(1000);
    let mut b = Budget::new(&mut w, exact);
    run_closed(&mut b, 1, construct, emit).unwrap();
    assert_eq!(b.peak_storage(), exact);
    assert_eq!(b.storage(), 0);
    let mut w = Work::new(1000);
    let mut b = Budget::new(&mut w, exact - 1);
    let got = run_closed(&mut b, 1, construct, emit);
    assert!(matches!(
        got,
        Err(PrivateBf16LlvmErrorV1::RankedVerification(
            E::ConditionalResource(Resource::Storage(_))
        ))
    ));
    assert_eq!(b.storage(), 0);
}
#[test]
fn foreign_slot_ledger_and_unknown_surplus_never_guess_a_refund() {
    let mut other_work = Work::new(1000);
    let other = Budget::new(&mut other_work, 1000);
    for fault in 0..3 {
        let mut work = Work::new(1000);
        let mut b = Budget::new(&mut work, 1000);
        b.reserve_storage(17).unwrap();
        let ledger = if fault == 0 {
            other.work_ledger_identity_v1()
        } else {
            b.work_ledger_identity_v1()
        };
        let slot = (&b as *const Budget<'_> as usize) + usize::from(fault == 1);
        let prepaid = if fault == 2 { 9 } else { 10 };
        let got = finish_closed(Ok(()), 7, ledger, slot, prepaid, &mut b);
        assert!(matches!(
            got,
            Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(Resource::Accounting)
            ))
        ));
        assert_eq!(b.storage(), 17);
    }
}
#[test]
fn descriptor_oracle_is_complete_bytes_not_recalculated_identity() {
    let expected = b"zero-digest exact original descriptor";
    assert_eq!(equal_descriptor(expected, expected), Ok(()));
    let mut foreign = expected.to_vec();
    let n = foreign.len();
    foreign[n - 1] ^= 1;
    assert_ne!(Sha256::digest(&foreign), Sha256::digest(expected));
    assert_eq!(equal_descriptor(&foreign, expected), Err(Failure::Join));
    assert_eq!(
        equal_descriptor(&expected[..n - 1], expected),
        Err(Failure::Join)
    );
}
#[test]
fn worker_limits_reserve_both_executions_and_refuse_expired_or_wide_inputs() {
    let limits = WorkerExecutionLimitsV1::new(Duration::from_secs(1), 1024, 1024).unwrap();
    let output = WorkerOutputConstraintsV1::new(1024).unwrap();
    assert_eq!(
        check_limits(&output, limits, Instant::now() + Duration::from_secs(5)),
        Ok(())
    );
    assert_eq!(
        check_limits(&output, limits, Instant::now()),
        Err(Failure::Limit)
    );
    assert_eq!(
        check_limits(
            &WorkerOutputConstraintsV1::new(OUTPUT_LIMIT + 1).unwrap(),
            limits,
            Instant::now() + Duration::from_secs(5)
        ),
        Err(Failure::Limit)
    );
    let wide = WorkerExecutionLimitsV1::new(Duration::from_secs(121), 1024, 1024).unwrap();
    assert_eq!(
        check_limits(&output, wide, Instant::now() + Duration::from_secs(600)),
        Err(Failure::Limit)
    );
}
#[test]
fn replay_runs_after_success_error_and_unwind_and_cannot_promote_failure() {
    for prior in [None, Some(ENGINE), Some(UNWIND)] {
        let calls = Cell::new(0);
        let attempted = prior.map_or(Ok(()), |reason| Err(mismatch(reason)));
        let got = after_replay(attempted, Ok(()), || {
            calls.set(calls.get() + 1);
            Ok(())
        });
        assert_eq!(calls.get(), 1);
        if let Some(reason) = prior {
            assert!(is_reason(&got, reason));
        } else {
            assert!(got.is_ok());
        }
    }
}
#[test]
fn replay_source_or_original_account_refusal_dominates_engine_failure() {
    let calls = Cell::new(0);
    let got = after_replay(Err(mismatch(ENGINE)), Ok(()), || {
        calls.set(1);
        Err(mismatch(JOIN))
    });
    assert!(is_reason(&got, JOIN));
    assert_eq!(calls.get(), 1);
    let mut denied_work = Work::new(0);
    let mut denied_budget = Budget::new(&mut denied_work, 0);
    let denied = denied_budget.charge_work(1).unwrap_err();
    assert!(matches!(denied, Resource::Work(_)));
    let got = after_replay(
        Err(mismatch(ENGINE)),
        Err(private_bf16_target_resource_v1(denied)),
        || {
            calls.set(2);
            Err(mismatch(JOIN))
        },
    );
    assert!(matches!(
        got,
        Err(PrivateBf16LlvmErrorV1::RankedVerification(
            E::ConditionalResource(Resource::Work(_))
        ))
    ));
    assert_eq!(calls.get(), 2);
}

#[test]
fn production_continuation_is_separate_from_existing_test_engine_and_public_dispatch() {
    let source = include_str!("bf16_same_owner_engineering_v1.rs");
    let owner = source
        .split("impl PrivateBf16WorkerHandoffV1 {")
        .nth(1)
        .unwrap();
    assert!(
        owner
            .find("self.revalidate_private_bf16_worker_handoff_v1")
            .unwrap()
            < owner.find("check_limits(").unwrap()
    );
    assert!(owner.contains("after_replay("));
    assert!(owner.contains("|| self.revalidate_private_bf16_worker_handoff_v1"));
    assert!(owner.contains("require_before_deadline(deadline, Instant::now())"));
    for forbidden in [
        "Budget::new",
        "Work::new",
        "for_test_v1(",
        "pub fn ",
        "-> Result<Row",
        "canonical_bytes(&self)",
        "external_providers",
        "WorkerRequestV3",
    ] {
        assert!(!owner.contains(forbidden), "{forbidden}");
    }
    assert!(
        source
            .split_whitespace()
            .collect::<String>()
            .contains("observe_engineering_hsaco_v1(handoff,worker,Vec::new(),output,limits)")
    );
    assert!(source.split_whitespace().collect::<String>().contains(
        "equal_descriptor(raw.get(loc.offset()..end).ok_or(Failure::Join)?,descriptor)?"
    ));
    let pipeline = include_str!("../production_pipeline/bf16_same_owner_engineering_v1.rs");
    let production = pipeline
        .split("/// Genuine-source negative controls only.")
        .next()
        .unwrap();
    assert!(production.contains("self.phase"));
    assert!(production.contains(".try_map("));
    assert!(
        production
            .find("observe_same_owner_engineering_v1")
            .unwrap()
            < production.find("drop(stage)").unwrap()
    );
    assert!(production.find("drop(stage)").unwrap() < production.find(".finish_copy()").unwrap());
    assert!(production.contains("attempted.map_err(Box::new)"));
    for forbidden in [
        "for_test_v1(",
        "Work::new",
        "Budget::new",
        "pub fn ",
        "WorkerRequestV3",
    ] {
        assert!(!production.contains(forbidden), "{forbidden}");
    }
    let public = include_str!("../production_pipeline/bf16_generated_source_admission_v1.rs");
    assert!(public.contains("ordinary.verify_general_kernel_checks()"));
    assert!(!public.contains("observe_engineering_and_drop_v1"));
}

#[test]
fn limits_are_exact_at_pair_and_postflight_deadlines_without_sleep() {
    let now = Instant::now();
    let output = WorkerOutputConstraintsV1::new(1024).unwrap();
    let limits = WorkerExecutionLimitsV1::new(Duration::from_secs(1), 1024, 1024).unwrap();
    let exact = now + Duration::from_secs(3);
    assert_eq!(check_limits_at(&output, limits, exact, now), Ok(()));
    assert_eq!(
        check_limits_at(&output, limits, exact - Duration::from_nanos(1), now),
        Err(Failure::Limit)
    );
    assert_eq!(require_before_deadline(exact, exact), Err(Failure::Limit));
    assert_eq!(
        require_before_deadline(exact, exact - Duration::from_nanos(1)),
        Ok(())
    );
    for (time, stdout, stderr) in [(121, 1024, 1024), (1, STREAM_LIMIT + 1, 1024)] {
        let bad = WorkerExecutionLimitsV1::new(Duration::from_secs(time), stdout, stderr).unwrap();
        assert_eq!(
            check_limits_at(&output, bad, now + Duration::from_secs(600), now),
            Err(Failure::Limit)
        );
    }
    // The worker constructor itself owns the stderr cap; the adapter cannot
    // receive a value exceeding it.
    let invalid_stderr =
        WorkerExecutionLimitsV1::new(Duration::from_secs(1), 1024, 65537).unwrap_err();
    assert_eq!(
        invalid_stderr.kind(),
        &fe2o3_hsaco_finalize::WorkerExecutionErrorKind::InvalidLimits
    );
    let maximum =
        WorkerExecutionLimitsV1::new(Duration::from_secs(120), STREAM_LIMIT, 65536).unwrap();
    assert_eq!(
        check_limits_at(&output, maximum, now + Duration::from_secs(241), now),
        Ok(())
    );
    let oversized_output = WorkerOutputConstraintsV1::new(4 * 1024 * 1024 + 1).unwrap();
    assert_eq!(
        check_limits_at(
            &oversized_output,
            limits,
            now + Duration::from_secs(600),
            now
        ),
        Err(Failure::Limit)
    );
    assert!(WorkerOutputConstraintsV1::new(0).is_err());
}

#[test]
fn selected_work_arithmetic_refuses_overflow_without_account_refill() {
    assert_eq!(selected_work(5, 7, 11).unwrap(), 23 * 32 + 1024);
    assert!(selected_work(usize::MAX, 1, 1).is_err());
    assert!(selected_work(1, usize::MAX, 1).is_err());
    assert!(selected_work(1, 1, u64::MAX).is_err());
}

#[test]
fn denied_entry_scope_never_fabricates_success_or_clears_outer_scope() {
    let scope = ObservationScope::begin(true).unwrap();
    assert!(ObservationScope::begin(false).is_err());
    assert_eq!(before_engine(), Err(Failure::EntryGuard));
    assert_eq!(scope.snapshot().engine_entries, 1);
    assert_eq!(scope.snapshot().collected_rows, 0);
    assert_eq!(scope.snapshot().postflights, 0);
    assert!(scope.snapshot().row.is_none());
    drop(scope);
    assert_eq!(before_engine(), Ok(()));
}

#[test]
fn observation_scope_is_thread_local_and_restores_on_unwind() {
    let scope = ObservationScope::begin(true).unwrap();
    std::thread::spawn(|| {
        assert_eq!(before_engine(), Ok(()));
        let caught = std::panic::catch_unwind(|| {
            let _local = ObservationScope::begin(true).unwrap();
            assert_eq!(before_engine(), Err(Failure::EntryGuard));
            std::panic::resume_unwind(Box::new("inert scope unwind"));
        });
        assert!(caught.is_err());
        drop(caught);
        assert_eq!(before_engine(), Ok(()));
    })
    .join()
    .unwrap();
    assert_eq!(scope.snapshot().engine_entries, 0);
}

#[test]
fn fixed_observation_does_not_become_success_before_postflight() {
    let scope = ObservationScope::begin(false).unwrap();
    assert_eq!(before_engine(), Ok(()));
    record_row(row());
    let captured = scope.snapshot();
    assert_eq!(
        (
            captured.engine_entries,
            captured.collected_rows,
            captured.postflights
        ),
        (1, 1, 0)
    );
    assert!(captured.row.is_some());
    record_postflight();
    assert_eq!(scope.snapshot().postflights, 1);
}

#[test]
fn owned_mutation_controls_use_real_entry_and_preserve_exact_six_refusals() {
    let source = include_str!("bf16_same_owner_engineering_v1_tests.rs");
    let controls = source
        .split("pub(crate) fn exercise_same_owner_engineering_refusals_for_test_v1(")
        .nth(1)
        .unwrap()
        .split("fn row()")
        .next()
        .unwrap();
    assert!(controls.contains("for fault in 0..6"));
    assert!(controls.contains("self.observe_same_owner_engineering_v1("));
    assert!(controls.contains("scope.entries() != Some(0)"));
    assert!(controls.contains("CorrespondenceMismatch"));
    assert!(controls.contains("ModuleIdentityMismatch"));
    assert!(controls.contains("self.same_owner_engineering_checkpoint_for_test_v1()?"));
    for forbidden in ["Budget::new", "observe_engineering_hsaco_v1(", "Work::new"] {
        assert!(!controls.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn prior_storage_denial_is_not_swallowed_by_a_successful_inert_closure() {
    let called = Cell::new(false);
    let mut work = Work::new(1000);
    let mut b = Budget::new(&mut work, 32);
    let original = b.reserve_storage(33).unwrap_err();
    let result = run_closed(
        &mut b,
        1,
        || {
            called.set(true);
            Ok(row())
        },
        |_| Ok(()),
    );
    assert!(!called.get());
    assert!(
        matches!(result, Err(PrivateBf16LlvmErrorV1::RankedVerification(
        E::ConditionalResource(error))) if error == original)
    );
    assert_eq!(b.work(), 0);
    assert_eq!(b.storage(), 0);
    assert_eq!(b.failed_storage(), Some(33));
}

#[test]
fn capture_and_panic_payload_die_before_known_credit_refund() {
    for mode in 0..3 {
        let scope = ObservationScope::begin(false).unwrap();
        let mut work = Work::new(if mode == 0 { 0 } else { 1000 });
        let mut b = Budget::new(&mut work, 4096);
        let capture = DropEvent(1);
        let result = run_closed(
            &mut b,
            7,
            move || {
                let _capture = capture;
                if mode == 2 {
                    std::panic::resume_unwind(Box::new(DropEvent(3)));
                }
                Ok(row())
            },
            |_| Ok(()),
        );
        let (events, count) = scope.events();
        if mode == 2 {
            assert!(is_reason(&result, UNWIND));
            assert_eq!(&events[..count], &[1, 3, 2]);
        } else {
            assert_eq!(&events[..count], &[1, 2]);
            assert_eq!(result.is_ok(), mode == 1);
        }
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn actual_policy_prepays_selected_maximum_or_refuses_before_entry() {
    let amount = selected_work(4096, 2048, OUTPUT_LIMIT).unwrap();
    assert_eq!(amount, 134_415_360);
    let policy = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
    assert_eq!(policy, 1usize << 54);
    assert!(amount < policy);
    // Logical accounting only: no correspondingly sized allocation or engine.
    let mut work = Work::new(policy);
    let mut budget = Budget::new(&mut work, 4096);
    budget.charge_work(17).unwrap();
    let original = budget.work_ledger_identity_v1();
    run_closed(&mut budget, amount, || Ok(row()), |_| Ok(())).unwrap();
    assert_eq!(budget.work(), 17 + amount);
    assert!(budget.work_ledger_identity_v1() == original);
    let entered = Cell::new(false);
    let mut short = Work::new(amount - 1);
    let mut budget = Budget::new(&mut short, 4096);
    let result = run_closed(
        &mut budget,
        amount,
        || {
            entered.set(true);
            Ok(row())
        },
        |_| Ok(()),
    );
    assert!(matches!(
        result,
        Err(PrivateBf16LlvmErrorV1::RankedVerification(
            E::ConditionalResource(Resource::Work(_))
        ))
    ));
    assert!(!entered.get());
    assert_eq!(budget.work(), 0);
    assert!(budget.check_prior_denials_v1().is_err());
}

#[test]
fn engine_failure_diagnostics_are_fixed_and_never_format_stream_payloads() {
    let source = include_str!("bf16_same_owner_engineering_v1.rs");
    assert!(
        source.contains("\"fe2o3-bf16-same-owner-engineering-inspection-error-v1 {INSPECTION}\"")
    );
    assert!(source.contains("\"fe2o3-bf16-same-owner-engineering-error-v1 {ENGINE}\""));
    assert!(!source.contains("{error:?}"));
    assert!(ENGINE.len() + INSPECTION.len() + 128 < 1024);
    assert_ne!(ENGINE, JOIN);
    assert_ne!(ENGINE, LIMIT);
}
