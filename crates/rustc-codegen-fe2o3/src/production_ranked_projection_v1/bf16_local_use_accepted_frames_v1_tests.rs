//! Fixed test-only observation of separate local DATA/source-use factories.
//! No production fields, allocator, ready token, operation cursor or admission.
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::thread::{self, ThreadId};

pub(in crate::production_ranked_projection_v1) const RUNS: usize = 12;
const EVENTS: usize = 5;
const RECORDS: usize = 69;
const MAX_LINE: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::production_ranked_projection_v1) struct Account {
    pub work: usize,
    pub storage: usize,
    pub peak: usize,
}
impl Account {
    const ZERO: Self = Self {
        work: 0,
        storage: 0,
        peak: 0,
    };
    fn valid(self) {
        assert!(self.peak >= self.storage, "local-use account peak");
    }
    fn follows(self, before: Self) {
        self.valid();
        assert!(
            self.work >= before.work && self.peak >= before.peak,
            "local-use account regression"
        );
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::production_ranked_projection_v1) enum FrameKind {
    RichHeader,
    LocalWrapper,
    SourceUseWrapper,
    Assembly,
    CompleteGraph,
    InitialGraph,
}
impl FrameKind {
    fn label(self) -> &'static str {
        match self {
            Self::RichHeader => "rich_header",
            Self::LocalWrapper => "local_wrapper",
            Self::SourceUseWrapper => "source_use_wrapper",
            Self::Assembly => "assembly",
            Self::CompleteGraph => "complete_graph",
            Self::InitialGraph => "initial_graph",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::production_ranked_projection_v1) enum Outcome {
    Observed,
    OccupiedRefused,
    CallbackError,
    CallbackPanic,
    ForeignQueryRefused,
    ForeignPendingRefused,
    CallRefused,
}
impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::OccupiedRefused => "occupied_refused",
            Self::CallbackError => "callback_error",
            Self::CallbackPanic => "callback_panic",
            Self::ForeignQueryRefused => "foreign_query_refused",
            Self::ForeignPendingRefused => "foreign_pending_refused",
            Self::CallRefused => "call_refused",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::production_ranked_projection_v1) struct Coordinate {
    pub block: usize,
    pub statement: Option<usize>,
    pub ordinal: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::production_ranked_projection_v1) struct WitnessSummary {
    pub locals: usize,
    pub blocks: usize,
    pub occurrences: usize,
    pub expected_some: usize,
    pub expected_none: usize,
    pub expected_refused: usize,
    pub call_boundaries: usize,
    pub some: Coordinate,
    pub none: Coordinate,
    pub call: Coordinate,
}
impl WitnessSummary {
    fn validate(self) {
        assert!(
            u32::try_from(self.locals).is_ok() && u32::try_from(self.blocks).is_ok(),
            "local-use source coordinate range"
        );
        assert!(
            self.locals > 0
                && self.blocks > 0
                && self.expected_some > 0
                && self.expected_none > 0
                && self.call_boundaries > 0,
            "local-use missing witness"
        );
        assert_eq!(
            add(
                add(self.expected_some, self.expected_none),
                self.expected_refused
            ),
            self.occurrences,
            "local-use occurrence partition"
        );
        assert!(
            self.some != self.none && self.some != self.call && self.none != self.call,
            "local-use duplicate witness"
        );
        for point in [self.some, self.none, self.call] {
            assert!(
                point.block < self.blocks && u32::try_from(point.block).is_ok(),
                "local-use witness block"
            );
            assert!(
                point.statement.is_none_or(|n| u32::try_from(n).is_ok()),
                "local-use witness statement"
            );
            assert!(
                point.ordinal < self.occurrences,
                "local-use witness ordinal"
            );
        }
        assert_eq!(
            (self.call.statement, self.call.ordinal),
            (None, 0),
            "local-use call coordinate"
        );
    }
}
#[derive(Clone, Copy)]
struct Frame {
    kind: FrameKind,
    bytes: usize,
}
#[derive(Clone, Copy)]
struct Run {
    before: Account,
    after: Account,
    frames: [Option<Frame>; EVENTS],
    len: usize,
    outcome: Option<Outcome>,
    entered: bool,
    access_frame: usize,
    origin_frame: usize,
}
impl Run {
    const EMPTY: Self = Self {
        before: Account::ZERO,
        after: Account::ZERO,
        frames: [None; EVENTS],
        len: 0,
        outcome: None,
        entered: false,
        access_frame: 0,
        origin_frame: 0,
    };
}
#[derive(Clone, Copy)]
struct State {
    active: bool,
    poisoned: bool,
    epoch: u64,
    owner: Option<ThreadId>,
    before: Account,
    rich: Option<usize>,
    runs: [Run; RUNS],
    finished: usize,
    current: Option<usize>,
}
impl State {
    const EMPTY: Self = Self {
        active: false,
        poisoned: false,
        epoch: 0,
        owner: None,
        before: Account::ZERO,
        rich: None,
        runs: [Run::EMPTY; RUNS],
        finished: 0,
        current: None,
    };
    fn check(&self, epoch: u64, owner: ThreadId) {
        assert!(
            self.active
                && !self.poisoned
                && self.epoch == epoch
                && self.owner == Some(owner)
                && owner == thread::current().id(),
            "local-use recorder custody"
        );
    }
}
thread_local! { static STATE: RefCell<State> = const { RefCell::new(State::EMPTY) }; }

fn add(a: usize, b: usize) -> usize {
    a.checked_add(b).expect("local-use arithmetic")
}
fn run_mode(slot: usize) -> &'static str {
    [
        "observe_all",
        "occupied",
        "callback_error",
        "callback_panic",
        "foreign_query_ledger",
        "observe_some",
        "observe_none",
        "occupied_some",
        "callback_error_some",
        "callback_panic_some",
        "foreign_pending_some",
        "call_refused",
    ][slot]
}
fn scope(slot: usize) -> &'static str {
    if slot < 5 {
        "local_contracts"
    } else {
        "source_use"
    }
}
fn expected_outcome(slot: usize) -> Outcome {
    match slot {
        0 | 5 | 6 => Outcome::Observed,
        1 | 7 => Outcome::OccupiedRefused,
        2 | 8 => Outcome::CallbackError,
        3 | 9 => Outcome::CallbackPanic,
        4 => Outcome::ForeignQueryRefused,
        10 => Outcome::ForeignPendingRefused,
        11 => Outcome::CallRefused,
        _ => panic!("local-use slot"),
    }
}
fn event_count(slot: usize) -> usize {
    if matches!(slot, 1 | 7 | 10) { 5 } else { 4 }
}
fn expected_frame(slot: usize, ordinal: usize) -> FrameKind {
    match ordinal {
        0 | 4 => {
            if slot < 5 {
                FrameKind::LocalWrapper
            } else {
                FrameKind::SourceUseWrapper
            }
        }
        1 => FrameKind::Assembly,
        2 => FrameKind::CompleteGraph,
        3 => FrameKind::InitialGraph,
        _ => panic!("local-use event ordinal"),
    }
}
fn frame_sum(run: &Run) -> usize {
    run.frames[..run.len].iter().fold(0, |sum, frame| {
        add(sum, frame.expect("local-use missing frame").bytes)
    })
}
/// Conservative fixed recorder/header and bounded diagnostic formatting envelope.
/// A caller must prepay this explicit logical storage; this is not process RSS.
pub(in crate::production_ranked_projection_v1) fn diagnostic_storage_bound() -> usize {
    add(
        add(
            std::mem::size_of::<State>()
                .checked_mul(3)
                .expect("local-use state storage"),
            std::mem::size_of::<EnvelopeGuard>()
                + std::mem::size_of::<RunGuard>()
                + std::mem::size_of::<WitnessSummary>() * 2,
        ),
        RECORDS * MAX_LINE,
    )
}
pub(in crate::production_ranked_projection_v1) fn active() -> bool {
    STATE.with(|cell| cell.borrow().active)
}
pub(in crate::production_ranked_projection_v1) struct EnvelopeGuard {
    epoch: u64,
    owner: ThreadId,
    finished: bool,
    not_send: PhantomData<Rc<()>>,
}
pub(in crate::production_ranked_projection_v1) struct RunGuard {
    epoch: u64,
    owner: ThreadId,
    slot: usize,
    finished: bool,
    not_send: PhantomData<Rc<()>>,
}
pub(in crate::production_ranked_projection_v1) fn start(before: Account) -> EnvelopeGuard {
    before.valid();
    let owner = thread::current().id();
    let epoch = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.active {
            state.poisoned = true;
        }
        assert!(!state.active, "local-use overlapping envelope");
        let epoch = state.epoch.checked_add(1).expect("local-use epoch");
        *state = State {
            active: true,
            epoch,
            owner: Some(owner),
            before,
            ..State::EMPTY
        };
        epoch
    });
    EnvelopeGuard {
        epoch,
        owner,
        finished: false,
        not_send: PhantomData,
    }
}
pub(in crate::production_ranked_projection_v1) fn begin(slot: usize, before: Account) -> RunGuard {
    let owner = thread::current().id();
    let epoch = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.check(state.epoch, owner);
        state.poisoned = true;
        assert!(
            slot < RUNS && slot == state.finished && state.current.is_none(),
            "local-use run order"
        );
        assert!(state.rich.is_some(), "local-use rich header missing");
        if slot == 0 {
            before.follows(state.before);
            assert!(
                before.storage >= add(state.before.storage, state.rich.unwrap()),
                "local-use rich floor"
            );
        } else {
            assert_eq!(
                before,
                state.runs[slot - 1].after,
                "local-use unaccounted inter-run gap"
            );
        }
        state.runs[slot].before = before;
        state.current = Some(slot);
        state.poisoned = false;
        state.epoch
    });
    RunGuard {
        epoch,
        owner,
        slot,
        finished: false,
        not_send: PhantomData,
    }
}
pub(in crate::production_ranked_projection_v1) fn record(kind: FrameKind, bytes: usize) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if !state.active {
            return;
        }
        state.check(state.epoch, thread::current().id());
        state.poisoned = true;
        assert!(bytes > 0, "local-use zero frame");
        if kind == FrameKind::RichHeader {
            assert!(
                state.rich.is_none() && state.finished == 0 && state.current.is_none(),
                "local-use rich event order"
            );
            state.rich = Some(bytes);
            state.poisoned = false;
            return;
        }
        let slot = state.current.expect("local-use frame outside run");
        let run = &mut state.runs[slot];
        assert!(run.len < event_count(slot), "local-use extra frame");
        assert_eq!(kind, expected_frame(slot, run.len), "local-use frame order");
        run.frames[run.len] = Some(Frame { kind, bytes });
        run.len += 1;
        state.poisoned = false;
    });
}
impl RunGuard {
    pub(in crate::production_ranked_projection_v1) fn finish(
        mut self,
        after: Account,
        outcome: Outcome,
        entered: bool,
        access_frame: usize,
        origin_frame: usize,
    ) {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.check(self.epoch, self.owner);
            assert_eq!(state.current, Some(self.slot), "local-use wrong run");
            let run = &mut state.runs[self.slot];
            assert_eq!(
                run.len,
                event_count(self.slot),
                "local-use incomplete frames"
            );
            assert_eq!(outcome, expected_outcome(self.slot), "local-use outcome");
            assert_eq!(entered, self.slot != 11, "local-use callback admission");
            assert!(
                access_frame > 0 && origin_frame > 0,
                "local-use missing stage frame"
            );
            after.follows(run.before);
            let frames = frame_sum(run);
            assert!(
                after.work - run.before.work >= frames,
                "local-use frame work floor"
            );
            assert!(
                after.storage
                    >= add(
                        run.before.storage,
                        add(frames, add(access_frame, origin_frame))
                    ),
                "local-use retained frame floor"
            );
            run.after = after;
            run.outcome = Some(outcome);
            run.entered = entered;
            run.access_frame = access_frame;
            run.origin_frame = origin_frame;
            state.finished += 1;
            state.current = None;
        });
        self.finished = true;
    }
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        if !self.finished {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                if state.active && state.epoch == self.epoch {
                    state.poisoned = true;
                }
            });
        }
    }
}
impl EnvelopeGuard {
    pub(in crate::production_ranked_projection_v1) fn finish(
        mut self,
        after_cleanup: Account,
        witnesses: WitnessSummary,
    ) {
        let snapshot = STATE.with(|cell| {
            let state = cell.borrow();
            state.check(self.epoch, self.owner);
            assert!(
                state.finished == RUNS && state.current.is_none(),
                "local-use incomplete schedule"
            );
            witnesses.validate();
            after_cleanup.follows(state.runs[RUNS - 1].after);
            assert_eq!(
                after_cleanup.storage, state.before.storage,
                "local-use refund floor"
            );
            *state
        });
        // Validate arithmetic BEFORE any success-family output.
        let setup = snapshot.runs[0].before.work - snapshot.before.work;
        let run_work = snapshot
            .runs
            .iter()
            .fold(0, |sum, run| add(sum, run.after.work - run.before.work));
        let teardown = after_cleanup.work - snapshot.runs[RUNS - 1].after.work;
        let work = after_cleanup.work - snapshot.before.work;
        assert_eq!(
            add(add(setup, run_work), teardown),
            work,
            "local-use work telescope"
        );
        let frames = snapshot
            .runs
            .iter()
            .fold(0, |sum, run| add(sum, frame_sum(run)));
        let rich = snapshot.rich.expect("local-use missing rich header");
        let all_storage = add(rich, frames);
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.check(self.epoch, self.owner);
            state.active = false;
        });
        self.finished = true;
        eprintln!(
            "fe2o3-local-use-accepted-v1 envelope runs=12 frames=52 locals={} blocks={} occurrences={} some={} none={} refused={} calls={} work_before={} storage_before={} peak_before={}",
            witnesses.locals,
            witnesses.blocks,
            witnesses.occurrences,
            witnesses.expected_some,
            witnesses.expected_none,
            witnesses.expected_refused,
            witnesses.call_boundaries,
            snapshot.before.work,
            snapshot.before.storage,
            snapshot.before.peak
        );
        for (role, point) in [
            ("some", witnesses.some),
            ("none", witnesses.none),
            ("call", witnesses.call),
        ] {
            match point.statement {
                Some(statement) => eprintln!(
                    "fe2o3-local-use-accepted-v1 witness role={} block={} statement={} ordinal={}",
                    role, point.block, statement, point.ordinal
                ),
                None => eprintln!(
                    "fe2o3-local-use-accepted-v1 witness role={} block={} statement=term ordinal={}",
                    role, point.block, point.ordinal
                ),
            }
        }
        eprintln!(
            "fe2o3-local-use-accepted-v1 frame slot=setup ordinal=0 kind=rich_header charge=storage bytes={}",
            rich
        );
        for (slot, run) in snapshot.runs.iter().enumerate() {
            for (ordinal, frame) in run.frames[..run.len].iter().enumerate() {
                let frame = frame.expect("validated frame");
                eprintln!(
                    "fe2o3-local-use-accepted-v1 frame slot={} ordinal={} kind={} charge=work_storage bytes={}",
                    slot,
                    ordinal,
                    frame.kind.label(),
                    frame.bytes
                );
            }
            eprintln!(
                "fe2o3-local-use-accepted-v1 run slot={} scope={} mode={} outcome={} entered={} work_before={} work_after={} storage_before={} storage_after={} peak_before={} peak_after={} access_frame={} origin_frame={}",
                slot,
                scope(slot),
                run_mode(slot),
                run.outcome.expect("validated outcome").label(),
                usize::from(run.entered),
                run.before.work,
                run.after.work,
                run.before.storage,
                run.after.storage,
                run.before.peak,
                run.after.peak,
                run.access_frame,
                run.origin_frame
            );
        }
        eprintln!(
            "fe2o3-local-use-accepted-v1 complete work={} setup_work={} run_work={} teardown_work={} work_after={} storage_after={} peak_after={} rich_storage={} frame_work={} frame_storage={}",
            work,
            setup,
            run_work,
            teardown,
            after_cleanup.work,
            after_cleanup.storage,
            after_cleanup.peak,
            rich,
            frames,
            all_storage
        );
    }
}
impl Drop for EnvelopeGuard {
    fn drop(&mut self) {
        if !self.finished {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                if state.epoch == self.epoch {
                    state.active = false;
                    state.poisoned = true;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn account(work: usize, storage: usize) -> Account {
        Account {
            work,
            storage,
            peak: storage,
        }
    }
    fn witnesses() -> WitnessSummary {
        WitnessSummary {
            locals: 8,
            blocks: 2,
            occurrences: 5,
            expected_some: 1,
            expected_none: 3,
            expected_refused: 1,
            call_boundaries: 1,
            some: Coordinate {
                block: 0,
                statement: Some(0),
                ordinal: 0,
            },
            none: Coordinate {
                block: 0,
                statement: Some(1),
                ordinal: 0,
            },
            call: Coordinate {
                block: 1,
                statement: None,
                ordinal: 0,
            },
        }
    }
    fn first() -> (EnvelopeGuard, Account) {
        let envelope = start(account(50, 40));
        record(FrameKind::RichHeader, 16);
        (envelope, account(100, 80))
    }
    fn frames(slot: usize) -> usize {
        let mut total = 0;
        for ordinal in 0..event_count(slot) {
            let bytes = 100 + slot * 10 + ordinal;
            record(expected_frame(slot, ordinal), bytes);
            total += bytes;
        }
        total
    }
    fn successful_run(slot: usize, before: Account) -> Account {
        let run = begin(slot, before);
        let frame_bytes = frames(slot);
        let after = account(
            before.work + frame_bytes + 17,
            before.storage + frame_bytes + 19,
        );
        run.finish(after, expected_outcome(slot), slot != 11, 3, 5);
        after
    }
    fn completed() -> (EnvelopeGuard, Account) {
        let (envelope, mut last) = first();
        for slot in 0..RUNS {
            last = successful_run(slot, last);
        }
        (
            envelope,
            Account {
                work: last.work + 7,
                storage: 40,
                peak: last.peak,
            },
        )
    }
    fn rejected(f: impl FnOnce()) {
        assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
        assert!(!active(), "test must tear down its envelope");
    }

    #[test]
    fn fixed_roster_is_twelve_runs_fifty_one_paid_plus_one_storage() {
        assert_eq!((0..RUNS).map(event_count).sum::<usize>(), 51);
        assert_eq!(RECORDS, 1 + 3 + 52 + 12 + 1);
        assert_eq!([1, 7, 10].map(event_count), [5, 5, 5]);
        assert!(diagnostic_storage_bound() >= RECORDS * MAX_LINE);
        assert!(diagnostic_storage_bound() < 128 * 1024);
    }
    #[test]
    fn complete_synthetic_schedule_accepts_distinct_per_run_sizes_and_outcomes() {
        let (envelope, after) = completed();
        envelope.finish(after, witnesses());
        assert!(!active());
    }
    #[test]
    fn inactive_recording_does_not_start_or_change_envelope() {
        record(FrameKind::Assembly, 0);
        record(FrameKind::RichHeader, usize::MAX);
        assert!(!active());
    }
    #[test]
    fn overlapping_envelope_refuses_and_original_can_drop() {
        rejected(|| {
            let _first = start(account(1, 1));
            let _second = start(account(1, 1));
        });
    }
    #[test]
    fn missing_and_duplicate_rich_headers_refuse() {
        rejected(|| {
            let _env = start(account(1, 1));
            let _run = begin(0, account(1, 1));
        });
        rejected(|| {
            let (_env, _) = first();
            record(FrameKind::RichHeader, 16);
        });
    }
    #[test]
    fn wrong_initial_slot_and_overlapping_run_refuse() {
        rejected(|| {
            let (_env, before) = first();
            let _run = begin(1, before);
        });
        rejected(|| {
            let (_env, before) = first();
            let _run = begin(0, before);
            let _other = begin(0, before);
        });
    }
    #[test]
    fn zero_unknown_order_and_extra_frames_refuse() {
        rejected(|| {
            let (_env, before) = first();
            let _run = begin(0, before);
            record(FrameKind::LocalWrapper, 0);
        });
        rejected(|| {
            let (_env, before) = first();
            let _run = begin(0, before);
            record(FrameKind::Assembly, 1);
        });
        rejected(|| {
            let (_env, before) = first();
            let _run = begin(0, before);
            frames(0);
            record(FrameKind::LocalWrapper, 1);
        });
    }
    #[test]
    fn missing_event_cannot_finish_a_run() {
        rejected(|| {
            let (_env, before) = first();
            let run = begin(0, before);
            record(FrameKind::LocalWrapper, 100);
            run.finish(account(1000, 1000), Outcome::Observed, true, 3, 5);
        });
    }
    #[test]
    fn wrong_outcome_and_callback_admission_refuse() {
        rejected(|| {
            let (_env, before) = first();
            let run = begin(0, before);
            frames(0);
            run.finish(account(1000, 1000), Outcome::CallbackError, true, 3, 5);
        });
        rejected(|| {
            let (_env, before) = first();
            let run = begin(0, before);
            frames(0);
            run.finish(account(1000, 1000), Outcome::Observed, false, 3, 5);
        });
    }
    #[test]
    fn work_and_retained_storage_one_short_refuse() {
        rejected(|| {
            let (_env, before) = first();
            let run = begin(0, before);
            let sum = frames(0);
            run.finish(
                account(before.work + sum - 1, before.storage + sum + 8),
                Outcome::Observed,
                true,
                3,
                5,
            );
        });
        rejected(|| {
            let (_env, before) = first();
            let run = begin(0, before);
            let sum = frames(0);
            run.finish(
                account(before.work + sum, before.storage + sum + 7),
                Outcome::Observed,
                true,
                3,
                5,
            );
        });
    }
    #[test]
    fn missing_nongeneric_stage_frame_refuses() {
        rejected(|| {
            let (_env, before) = first();
            let run = begin(0, before);
            frames(0);
            run.finish(account(1000, 1000), Outcome::Observed, true, 0, 5);
        });
    }
    #[test]
    fn unaccounted_inter_run_work_or_storage_gap_refuses() {
        for storage in [false, true] {
            rejected(|| {
                let (_env, before) = first();
                let mut after = successful_run(0, before);
                if storage {
                    after.storage += 1;
                    after.peak += 1;
                } else {
                    after.work += 1;
                }
                let _next = begin(1, after);
            });
        }
    }
    #[test]
    fn abandoned_run_poison_is_not_a_successful_callback_panic() {
        rejected(|| {
            let (_env, before) = first();
            let run = begin(0, before);
            frames(0);
            drop(run);
            let _next = begin(0, before);
        });
    }
    #[test]
    fn incomplete_envelope_cannot_print_success() {
        rejected(|| {
            let (env, _) = first();
            env.finish(account(100, 40), witnesses());
        });
    }
    #[test]
    fn final_refund_floor_and_peak_regression_refuse() {
        rejected(|| {
            let (env, mut after) = completed();
            after.storage += 1;
            env.finish(after, witnesses());
        });
        rejected(|| {
            let (env, mut after) = completed();
            after.peak = after.storage;
            env.finish(after, witnesses());
        });
    }
    #[test]
    fn impossible_witness_partition_and_coordinates_refuse() {
        for which in 0..5 {
            rejected(|| {
                let (env, after) = completed();
                let mut summary = witnesses();
                match which {
                    0 => summary.expected_refused += 1,
                    1 => summary.some = summary.none,
                    2 => summary.some.block = summary.blocks,
                    3 => summary.call.statement = Some(0),
                    _ => summary.none.ordinal = summary.occurrences,
                }
                env.finish(after, summary);
            });
        }
    }
    #[test]
    fn dropped_envelope_clears_active_state_without_printing_success() {
        let (env, before) = first();
        let run = begin(0, before);
        drop(run);
        drop(env);
        assert!(!active());
        let (next, _) = first();
        drop(next);
        assert!(!active());
    }
    #[test]
    fn thread_local_recording_is_isolated_and_foreign_owner_refuses() {
        let (env, _) = first();
        let snapshot = STATE.with(|cell| *cell.borrow());
        std::thread::spawn(move || {
            assert!(!active());
            assert!(
                catch_unwind(|| snapshot.check(snapshot.epoch, snapshot.owner.unwrap())).is_err()
            );
            record(FrameKind::Assembly, 1);
            assert!(!active());
        })
        .join()
        .unwrap();
        assert!(active());
        drop(env);
    }
    #[test]
    fn stale_epoch_guard_cannot_finish_a_new_envelope() {
        rejected(|| {
            let (old, _) = first();
            let stale = RunGuard {
                epoch: old.epoch,
                owner: old.owner,
                slot: 0,
                finished: false,
                not_send: PhantomData,
            };
            drop(old);
            let (_new, _) = first();
            stale.finish(account(1000, 1000), Outcome::Observed, true, 3, 5);
        });
    }
    #[test]
    fn caught_extra_frame_cannot_masquerade_as_expected_callback_panic() {
        rejected(|| {
            let (_env, mut before) = first();
            for slot in 0..3 {
                before = successful_run(slot, before);
            }
            let run = begin(3, before);
            frames(3);
            assert!(catch_unwind(|| record(FrameKind::LocalWrapper, 1)).is_err());
            run.finish(
                account(before.work + 10000, before.storage + 10000),
                Outcome::CallbackPanic,
                true,
                3,
                5,
            );
        });
    }
    #[test]
    fn caught_overlap_or_order_violation_poison_the_original_envelope() {
        rejected(|| {
            let (_env, before) = first();
            assert!(catch_unwind(|| start(account(1, 1))).is_err());
            let _run = begin(0, before);
        });
        rejected(|| {
            let (_env, before) = first();
            assert!(catch_unwind(|| begin(1, before)).is_err());
            let _run = begin(0, before);
        });
    }
    #[test]
    fn checked_arithmetic_refuses_overflow() {
        assert!(catch_unwind(|| add(usize::MAX, 1)).is_err());
        rejected(|| {
            let env = start(account(0, usize::MAX));
            record(FrameKind::RichHeader, 1);
            let _run = begin(0, account(1, usize::MAX));
            drop(env);
        });
    }
}
