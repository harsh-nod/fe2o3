//! Fixed diagnostic-only storage. This module is cfg(test), never recipe authority.
//! Accepted debits are observed only AFTER work and storage both succeeded.
//! No resource debit, refund, allocator call, runtime field or callback capture.
use std::cell::Cell;
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum Scope {
    S3,
    S4,
    S5A,
}
impl Scope {
    fn name(self) -> &'static str {
        match self {
            Self::S3 => "s3",
            Self::S4 => "s4",
            Self::S5A => "s5a",
        }
    }
    fn modes(self) -> &'static [Mode] {
        match self {
            Self::S3 | Self::S4 => &NINE,
            Self::S5A => &FIVE,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum Mode {
    Observe,
    Occupied,
    Error,
    Panic,
    MissingInputs,
    DuplicateInputs,
    ChangedBinding,
    EqualInputClone,
    ForeignPendingLedger,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Occupied => "occupied",
            Self::Error => "error",
            Self::Panic => "panic",
            Self::MissingInputs => "missing_inputs",
            Self::DuplicateInputs => "duplicate_inputs",
            Self::ChangedBinding => "changed_binding",
            Self::EqualInputClone => "equal_input_clone",
            Self::ForeignPendingLedger => "foreign_pending_ledger",
        }
    }
    fn admitted(self) -> bool {
        !matches!(
            self,
            Self::MissingInputs | Self::DuplicateInputs | Self::ChangedBinding
        )
    }
}
const NINE: [Mode; 9] = [
    Mode::Observe,
    Mode::Occupied,
    Mode::Error,
    Mode::Panic,
    Mode::MissingInputs,
    Mode::DuplicateInputs,
    Mode::ChangedBinding,
    Mode::EqualInputClone,
    Mode::ForeignPendingLedger,
];
const FIVE: [Mode; 5] = [
    Mode::Observe,
    Mode::Occupied,
    Mode::Error,
    Mode::Panic,
    Mode::ForeignPendingLedger,
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum Kind {
    Assembly,
    CompleteGraph,
    InitialGraph,
}
const KINDS: [Kind; 3] = [Kind::Assembly, Kind::CompleteGraph, Kind::InitialGraph];
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Assembly => "assembly",
            Self::CompleteGraph => "complete_graph",
            Self::InitialGraph => "initial_graph",
        }
    }
}
#[derive(Clone, Copy)]
struct Run {
    mode: Mode,
    credits: [usize; 3],
    len: usize,
    stage_seen: bool,
}
const EMPTY_RUN: Run = Run {
    mode: Mode::Observe,
    credits: [0; 3],
    len: 0,
    stage_seen: false,
};
#[derive(Clone, Copy)]
struct State {
    scope: Option<Scope>,
    runs: [Run; 9],
    finished: usize,
    current: Option<usize>,
    active: bool,
    invalid: bool,
}
impl State {
    const fn empty() -> Self {
        Self {
            scope: None,
            runs: [EMPTY_RUN; 9],
            finished: 0,
            current: None,
            active: false,
            invalid: false,
        }
    }
}
thread_local! { static STATE: Cell<State> = const { Cell::new(State::empty()) }; }

pub(in crate::production_ranked_projection_v1) fn start(scope: Scope) {
    STATE.with(|cell| {
        let mut old = cell.get();
        if old.scope.is_some() || old.active || old.current.is_some() {
            old.invalid = true;
            cell.set(old);
            panic!("accepted frame observer overlap");
        }
        cell.set(State {
            scope: Some(scope),
            ..State::empty()
        });
    });
}
// Private non-Send, non-Sync zero-sized guards. Neither is referenced by a
// measured callback: RunGuard lives outside it, StageGuard is created inside it.
pub(in crate::production_ranked_projection_v1) struct RunGuard(PhantomData<*mut ()>);
pub(in crate::production_ranked_projection_v1) struct StageGuard(PhantomData<*mut ()>);

pub(in crate::production_ranked_projection_v1) fn begin(mode: Mode) -> RunGuard {
    STATE.with(|cell| {
        let mut state = cell.get();
        let valid = state
            .scope
            .is_some_and(|scope| scope.modes().get(state.finished) == Some(&mode))
            && state.current.is_none()
            && !state.active
            && !state.invalid;
        if !valid {
            state.invalid = true;
            cell.set(state);
            panic!("accepted frame run order");
        }
        state.current = Some(state.finished);
        state.runs[state.finished].mode = mode;
        cell.set(state);
    });
    RunGuard(PhantomData)
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        STATE.with(|cell| {
            let mut state = cell.get();
            if let Some(index) = state.current.take() {
                let run = state.runs[index];
                if state.active
                    || run.len != if run.mode.admitted() { 3 } else { 0 }
                    || run.stage_seen != run.mode.admitted()
                {
                    state.invalid = true;
                }
                state.finished += 1;
            } else {
                state.invalid = true;
            }
            state.active = false;
            cell.set(state);
        });
    }
}
pub(in crate::production_ranked_projection_v1) fn enter() -> StageGuard {
    STATE.with(|cell| {
        let mut state = cell.get();
        let valid = state
            .current
            .is_some_and(|i| state.runs[i].mode.admitted() && !state.runs[i].stage_seen)
            && !state.active
            && !state.invalid;
        if !valid {
            state.invalid = true;
            cell.set(state);
            panic!("accepted frame stage overlap");
        }
        state.runs[state.current.unwrap()].stage_seen = true;
        state.active = true;
        cell.set(state);
    });
    StageGuard(PhantomData)
}
impl Drop for StageGuard {
    fn drop(&mut self) {
        STATE.with(|cell| {
            let mut state = cell.get();
            if !state.active || !state.current.is_some_and(|i| state.runs[i].len == 3) {
                state.invalid = true;
            }
            state.active = false;
            cell.set(state);
        });
    }
}
pub(in crate::production_ranked_projection_v1) fn record(kind: Kind, bytes: usize) {
    STATE.with(|cell| {
        let mut state = cell.get();
        // Hooks also execute in unrelated graph controls outside these observers.
        // They must not manufacture a scope or leak a row into the next observer.
        if state.scope.is_none() {
            return;
        }
        match state.current {
            Some(index) if state.active => {
                let run = &mut state.runs[index];
                if run.len >= 3 || KINDS[run.len] != kind || bytes == 0 {
                    state.invalid = true;
                } else {
                    run.credits[run.len] = bytes;
                    run.len += 1;
                }
            }
            _ => state.invalid = true,
        }
        cell.set(state);
    });
}
fn valid(state: &State, scope: Scope) -> bool {
    if state.scope != Some(scope)
        || state.invalid
        || state.active
        || state.current.is_some()
        || state.finished != scope.modes().len()
    {
        return false;
    }
    let first = state.runs[0].credits;
    for (index, mode) in scope.modes().iter().copied().enumerate() {
        let run = state.runs[index];
        if run.mode != mode
            || run.stage_seen != mode.admitted()
            || run.len != if mode.admitted() { 3 } else { 0 }
        {
            return false;
        }
        if mode.admitted() {
            if run.credits != first || run.credits.contains(&0) {
                return false;
            }
        } else if run.credits != [0; 3] {
            return false;
        }
    }
    true
}
pub(in crate::production_ranked_projection_v1) fn flush(
    scope: Scope,
    work: usize,
    assembly: usize,
) {
    let state = STATE.with(Cell::get);
    assert!(valid(&state, scope), "incomplete accepted frame observer");
    let per_run = state.runs[0]
        .credits
        .iter()
        .try_fold(0usize, |sum, value| sum.checked_add(*value))
        .expect("accepted frame credit sum overflow");
    let admitted_count = scope.modes().iter().filter(|mode| mode.admitted()).count();
    let accepted_work = per_run
        .checked_mul(admitted_count)
        .expect("accepted frame work overflow");
    assert!(
        work > 0 && work >= accepted_work && state.runs[0].credits[0] == assembly,
        "accepted assembly/work join"
    );
    // Clear only after validation. Every line is a finite closed diagnostic; the
    // actual original-ledger work was measured by the existing observer.
    STATE.with(|cell| cell.set(State::empty()));
    let mut admitted = 0usize;
    for (index, mode) in scope.modes().iter().copied().enumerate() {
        let run = state.runs[index];
        eprintln!(
            "fe2o3-root-accepted-frame-run-v1 scope={} run={} mode={} events={}",
            scope.name(),
            index,
            mode.name(),
            run.len
        );
        if mode.admitted() {
            admitted += 1;
        }
        for (ordinal, kind) in KINDS.iter().copied().enumerate().take(run.len) {
            eprintln!(
                "fe2o3-root-accepted-frame-v1 scope={} run={} mode={} kind={} bytes={}",
                scope.name(),
                index,
                mode.name(),
                kind.name(),
                run.credits[ordinal]
            );
        }
    }
    eprintln!(
        "fe2o3-root-accepted-frame-summary-v1 scope={} runs={} admitted={} events={} work={}",
        scope.name(),
        state.finished,
        admitted,
        admitted * 3,
        work
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    fn clear() {
        STATE.with(|cell| cell.set(State::empty()));
    }
    fn frames(values: [usize; 3]) {
        for (kind, bytes) in KINDS.into_iter().zip(values) {
            record(kind, bytes);
        }
    }
    fn complete(scope: Scope, values: [usize; 3]) {
        start(scope);
        for &mode in scope.modes() {
            let _run = begin(mode);
            if mode.admitted() {
                let _stage = enter();
                frames(values);
            }
        }
    }
    #[test]
    fn scoped_frames_fixed_storage_and_zero_sized_guards() {
        assert!(size_of::<State>() <= 1024);
        assert_eq!(size_of::<RunGuard>(), 0);
        assert_eq!(size_of::<StageGuard>(), 0);
        assert_eq!(NINE.iter().filter(|m| m.admitted()).count(), 6);
        assert_eq!(FIVE.iter().filter(|m| m.admitted()).count(), 5);
    }
    #[test]
    fn scoped_frames_closed_distinct_scope_counts() {
        for scope in [Scope::S3, Scope::S4, Scope::S5A] {
            clear();
            complete(scope, [11, 13, 17]);
            assert!(valid(&STATE.with(Cell::get), scope));
            assert_eq!(STATE.with(Cell::get).finished, scope.modes().len());
        }
        clear();
    }
    #[test]
    fn scoped_frames_ignores_unscoped_hooks_but_rejects_inactive_scoped_events() {
        clear();
        frames([1, 2, 3]);
        assert!(STATE.with(Cell::get).scope.is_none());
        start(Scope::S3);
        record(Kind::Assembly, 1);
        assert!(STATE.with(Cell::get).invalid);
        clear();
    }
    #[test]
    fn scoped_frames_rejects_nested_observers_runs_and_stages() {
        clear();
        start(Scope::S3);
        assert!(catch_unwind(|| start(Scope::S4)).is_err());
        assert!(STATE.with(Cell::get).invalid);
        clear();
        start(Scope::S3);
        let run = begin(Mode::Observe);
        assert!(catch_unwind(|| begin(Mode::Occupied)).is_err());
        drop(run);
        clear();
        start(Scope::S3);
        let run = begin(Mode::Observe);
        let stage = enter();
        assert!(catch_unwind(enter).is_err());
        drop(stage);
        drop(run);
        assert!(STATE.with(Cell::get).invalid);
        clear();
    }
    #[test]
    fn scoped_frames_rejects_reordered_modes_and_overflow() {
        clear();
        start(Scope::S5A);
        assert!(catch_unwind(|| begin(Mode::Occupied)).is_err());
        clear();
        complete(Scope::S5A, [1, 2, 3]);
        assert!(catch_unwind(|| begin(Mode::ForeignPendingLedger)).is_err());
        clear();
    }
    #[test]
    fn scoped_frames_rejects_zero_reorder_and_fourth_frame() {
        for bad in 0..3 {
            clear();
            start(Scope::S3);
            let run = begin(Mode::Observe);
            let stage = enter();
            match bad {
                0 => record(Kind::Assembly, 0),
                1 => record(Kind::InitialGraph, 1),
                _ => {
                    frames([1, 2, 3]);
                    record(Kind::Assembly, 4);
                }
            }
            drop(stage);
            drop(run);
            assert!(STATE.with(Cell::get).invalid);
        }
        clear();
    }
    #[test]
    fn scoped_frames_partial_drop_is_retained_and_invalid() {
        clear();
        start(Scope::S3);
        {
            let _run = begin(Mode::Observe);
            let _stage = enter();
            record(Kind::Assembly, 11);
        }
        let state = STATE.with(Cell::get);
        assert!(state.invalid && !state.active && state.current.is_none());
        assert_eq!(state.runs[0].credits, [11, 0, 0]);
        assert!(!valid(&state, Scope::S3));
        clear();
    }
    #[test]
    fn scoped_frames_panic_deactivates_with_complete_triplet_retained() {
        clear();
        start(Scope::S3);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _run = begin(Mode::Observe);
            let _stage = enter();
            frames([11, 13, 17]);
            panic!("callback");
        }));
        let state = STATE.with(Cell::get);
        assert!(!state.invalid && !state.active && state.current.is_none());
        assert_eq!(state.runs[0].credits, [11, 13, 17]);
        assert_eq!(state.finished, 1);
        clear();
    }
    #[test]
    fn scoped_frames_rejects_missing_or_changed_complete_trace() {
        clear();
        complete(Scope::S3, [11, 13, 17]);
        let state = STATE.with(Cell::get);
        let mut short = state;
        short.finished -= 1;
        assert!(!valid(&short, Scope::S3));
        let mut changed = state;
        changed.runs[1].credits[1] += 1;
        assert!(!valid(&changed, Scope::S3));
        assert!(!valid(&state, Scope::S4));
        clear();
    }
    #[test]
    fn scoped_frames_refused_attempt_cannot_admit_a_stage() {
        clear();
        start(Scope::S3);
        for &mode in &NINE[..4] {
            let _run = begin(mode);
            let _stage = enter();
            frames([11, 13, 17]);
        }
        let run = begin(Mode::MissingInputs);
        assert!(catch_unwind(enter).is_err());
        drop(run);
        assert!(STATE.with(Cell::get).invalid);
        clear();
    }
    #[test]
    fn scoped_frames_do_not_cross_threads() {
        clear();
        start(Scope::S3);
        std::thread::spawn(|| {
            assert!(STATE.with(Cell::get).scope.is_none());
            frames([1, 2, 3]);
        })
        .join()
        .unwrap();
        assert_eq!(STATE.with(Cell::get).scope, Some(Scope::S3));
        assert!(!STATE.with(Cell::get).invalid);
        clear();
    }
    #[test]
    fn scoped_frames_flush_requires_actual_assembly_and_work() {
        for (work, assembly) in [(0, 11), (5, 12)] {
            clear();
            complete(Scope::S5A, [11, 13, 17]);
            assert!(catch_unwind(|| flush(Scope::S5A, work, assembly)).is_err());
        }
        clear();
        complete(Scope::S5A, [11, 13, 17]);
        flush(Scope::S5A, 1234, 11);
        assert!(STATE.with(Cell::get).scope.is_none());
    }
}
