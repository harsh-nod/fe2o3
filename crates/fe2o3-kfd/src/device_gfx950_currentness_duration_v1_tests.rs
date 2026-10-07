use super::super::duration::{
    Clock, Gfx950EngineeringCurrentnessCallDurationV1 as Call,
    Gfx950EngineeringCurrentnessDurationsV1 as Totals, Measured,
};
use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Tick {
    origin: Instant,
    index: usize,
    calls: Rc<Cell<usize>>,
    panic_at: Option<usize>,
    backwards_at: Option<usize>,
}
impl Tick {
    fn new(origin: Instant) -> Self {
        Self {
            origin,
            index: 0,
            calls: Rc::new(Cell::new(0)),
            panic_at: None,
            backwards_at: None,
        }
    }
}
impl Clock for Tick {
    fn now(&mut self) -> Instant {
        let index = self.index;
        self.index += 1;
        self.calls.set(self.index);
        assert_ne!(
            self.panic_at,
            Some(index),
            "injected diagnostic clock unwind"
        );
        if self.backwards_at == Some(index) {
            return self.origin.checked_sub(Duration::from_nanos(1)).unwrap();
        }
        self.origin
            .checked_add(Duration::from_nanos(index as u64 * 7))
            .unwrap()
    }
}

struct Watch<'a> {
    inner: &'a mut Recording,
    deadlines: Rc<RefCell<Vec<usize>>>,
    expire_on_validation: bool,
    expired: Cell<bool>,
}
impl CheckpointBackend for Watch<'_> {
    type Snapshot = Snapshot;
    fn now(&mut self) -> Instant {
        self.deadlines.borrow_mut().push(self.inner.events.len());
        if self.expired.get() {
            self.inner.until
        } else {
            self.inner.now()
        }
    }
    fn participants(&self) -> usize {
        self.inner.participants()
    }
    fn identity(&mut self, rank: usize) -> Result<u64, DeviceBindingError> {
        self.inner.identity(rank)
    }
    fn before(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        self.inner.before(rank)
    }
    fn discover(&mut self) -> Result<Snapshot, DeviceBindingError> {
        self.inner.discover()
    }
    fn compare_retained(
        &mut self,
        rank: usize,
        snapshot: &Snapshot,
    ) -> Result<(), DeviceBindingError> {
        self.inner.compare_retained(rank, snapshot)
    }
    fn after(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        self.inner.after(rank)
    }
    fn root_generation(&mut self, snapshot: &Snapshot) -> Result<(), DeviceBindingError> {
        self.inner.root_generation(snapshot)
    }
    fn poison_all(&mut self) {
        self.inner.poison_all();
    }
    fn validate_durations(&self, counts: &ScopedCountsV1) -> Result<(), DeviceBindingError> {
        self.inner.validate_durations(counts)?;
        self.expired.set(self.expire_on_validation);
        Ok(())
    }
}

fn run<B: CheckpointBackend<Snapshot = Snapshot>>(
    backend: &mut B,
    until: Instant,
) -> Result<ScopedCountsV1, DeviceBindingError> {
    let mut window = Window::enter(backend, until)?;
    window.checkpoint(backend)?;
    window.checkpoint(backend)?;
    window.finish(backend)
}

fn measured(
    backend: &mut Recording,
    totals: &mut Totals,
    clock: Tick,
) -> Result<ScopedCountsV1, DeviceBindingError> {
    let until = backend.until;
    run(&mut Measured::new(backend, totals, clock), until)
}

fn pair(totals: &mut Totals, index: usize) -> &mut Call {
    match index {
        0 => &mut totals.before,
        1 => &mut totals.discover,
        2 => &mut totals.after,
        3 => &mut totals.root_generation,
        _ => panic!("four closed categories"),
    }
}

#[test]
fn duration_real_adapter_matches_unmeasured_predicate_and_deadline_order() {
    let mut plain = Recording::new();
    let until = plain.until;
    let old_deadlines = Rc::new(RefCell::new(Vec::new()));
    let old = run(
        &mut Watch {
            inner: &mut plain,
            deadlines: old_deadlines.clone(),
            expire_on_validation: false,
            expired: Cell::new(false),
        },
        until,
    )
    .unwrap();
    let mut backend = Recording::new();
    let until = backend.until;
    let clock = Tick::new(backend.now);
    let clock_calls = clock.calls.clone();
    let new_deadlines = Rc::new(RefCell::new(Vec::new()));
    let mut watch = Watch {
        inner: &mut backend,
        deadlines: new_deadlines.clone(),
        expire_on_validation: false,
        expired: Cell::new(false),
    };
    let mut totals = Totals::default();
    let counts = run(&mut Measured::new(&mut watch, &mut totals, clock), until).unwrap();
    assert_eq!(counts, old);
    assert_eq!(backend.events, plain.events);
    assert_eq!(*new_deadlines.borrow(), *old_deadlines.borrow());
    assert_eq!(
        totals.before,
        Call {
            calls: 8,
            elapsed_ns: 56
        }
    );
    assert_eq!(
        totals.discover,
        Call {
            calls: 2,
            elapsed_ns: 14
        }
    );
    assert_eq!(
        totals.after,
        Call {
            calls: 8,
            elapsed_ns: 56
        }
    );
    assert_eq!(
        totals.root_generation,
        Call {
            calls: 7,
            elapsed_ns: 49
        }
    );
    assert_eq!(clock_calls.get(), 50);
    assert_eq!(totals.checked_sum_ns().unwrap(), 175);
}

#[test]
fn duration_selected_rank_and_group_checkpoints_share_one_recorder() {
    let mut backend = Recording::new();
    let until = backend.until;
    let mut totals = Totals::default();
    let clock = Tick::new(backend.now);
    let mut window =
        Window::enter(&mut Measured::new(&mut backend, &mut totals, clock), until).unwrap();
    backend.count = 1;
    backend.ids[0] = 22;
    let clock = Tick::new(backend.now);
    window
        .checkpoint_rank(&mut Measured::new(&mut backend, &mut totals, clock), 1)
        .unwrap();
    backend.count = 2;
    backend.ids = [11, 22];
    let clock = Tick::new(backend.now);
    window
        .checkpoint(&mut Measured::new(&mut backend, &mut totals, clock))
        .unwrap();
    let clock = Tick::new(backend.now);
    let counts = window
        .finish(&mut Measured::new(&mut backend, &mut totals, clock))
        .unwrap();
    assert_eq!(counts.before_calls, 7);
    assert_eq!(counts.after_calls, 7);
    assert_eq!(counts.local_checkpoints, 2);
    assert_eq!(
        totals.before,
        Call {
            calls: 7,
            elapsed_ns: 49
        }
    );
    assert_eq!(
        totals.after,
        Call {
            calls: 7,
            elapsed_ns: 49
        }
    );
    assert_eq!(totals.discover.calls, 2);
    assert_eq!(totals.root_generation.calls, 7);
    assert_eq!(backend.poisoned, [false; 2]);
}

#[test]
fn duration_every_callback_fault_keeps_original_prefix_and_quarantine() {
    let mut control = Recording::new();
    exercise(&mut control).unwrap();
    for index in 0..control.events.len() {
        let mut backend = Recording::new();
        backend.fail_at = Some(index);
        let clock = Tick::new(backend.now);
        assert!(measured(&mut backend, &mut Totals::default(), clock).is_err());
        assert_eq!(&backend.events[..=index], &control.events[..=index]);
        assert_eq!(backend.events.len(), index + 2);
        assert_eq!(backend.events.last().unwrap(), "poison_all");
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn duration_every_callback_unwind_keeps_original_prefix_and_quarantine() {
    let mut control = Recording::new();
    exercise(&mut control).unwrap();
    for index in 0..control.events.len() {
        let mut backend = Recording::new();
        backend.panic_at = Some(index);
        let clock = Tick::new(backend.now);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                measured(&mut backend, &mut Totals::default(), clock)
            }))
            .is_err()
        );
        assert_eq!(&backend.events[..=index], &control.events[..=index]);
        assert_eq!(backend.events.len(), index + 2);
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn duration_clock_unwind_before_and_after_each_callback_never_commits() {
    let mut control = Recording::new();
    exercise(&mut control).unwrap();
    for index in 0..50 {
        let mut backend = Recording::new();
        let mut clock = Tick::new(backend.now);
        clock.panic_at = Some(index);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                measured(&mut backend, &mut Totals::default(), clock)
            }))
            .is_err()
        );
        let observed = backend.events.len() - 1;
        assert_eq!(&backend.events[..observed], &control.events[..observed]);
        assert_eq!(backend.events.last().unwrap(), "poison_all");
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn duration_backwards_clock_refuses_after_callback_without_later_observation() {
    let mut backend = Recording::new();
    let mut clock = Tick::new(backend.now);
    clock.backwards_at = Some(1);
    let mut totals = Totals::default();
    assert!(measured(&mut backend, &mut totals, clock).is_err());
    assert_eq!(totals, Totals::default());
    assert!(backend.events.iter().any(|v| v == "before:0:apertures"));
    assert!(!backend.events.iter().any(|v| v.starts_with("before:1:")));
    assert_eq!(backend.poisoned, [true; 2]);
}

#[test]
fn duration_each_call_and_elapsed_overflow_refuses_under_engine_guard() {
    for index in 0..4 {
        for overflow_calls in [false, true] {
            let mut backend = Recording::new();
            let clock = Tick::new(backend.now);
            let mut totals = Totals::default();
            if overflow_calls {
                pair(&mut totals, index).calls = u64::MAX;
            } else {
                pair(&mut totals, index).elapsed_ns = u64::MAX - 6;
            }
            let before = *pair(&mut totals, index);
            assert!(measured(&mut backend, &mut totals, clock).is_err());
            assert_eq!(*pair(&mut totals, index), before);
            assert_eq!(backend.poisoned, [true; 2]);
        }
    }
}

#[test]
fn duration_count_mismatch_and_category_sum_refuse_before_finish_commit() {
    for index in 0..5 {
        let mut backend = Recording::new();
        let clock = Tick::new(backend.now);
        let mut totals = Totals::default();
        if index < 4 {
            pair(&mut totals, index).calls = 1;
        } else {
            totals.before.elapsed_ns = u64::MAX / 2;
            totals.after.elapsed_ns = u64::MAX / 2;
        }
        assert!(measured(&mut backend, &mut totals, clock).is_err());
        assert_eq!(
            backend.events.iter().filter(|v| *v == "discover").count(),
            2
        );
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn duration_final_deadline_is_rechecked_after_reconciliation() {
    let mut backend = Recording::new();
    let until = backend.until;
    let clock = Tick::new(backend.now);
    let mut watch = Watch {
        inner: &mut backend,
        deadlines: Rc::new(RefCell::new(Vec::new())),
        expire_on_validation: true,
        expired: Cell::new(false),
    };
    let mut totals = Totals::default();
    assert!(run(&mut Measured::new(&mut watch, &mut totals, clock), until).is_err());
    assert_eq!(totals.discover.calls, 2);
    assert_eq!(backend.poisoned, [true; 2]);
}

#[test]
fn duration_elapsed_and_bank_subtotal_bounds_are_checked_integer_data() {
    let start = Instant::now();
    let end = start.checked_add(Duration::from_nanos(100)).unwrap();
    let mut totals = Totals::default();
    totals.before.elapsed_ns = 30;
    totals.discover.elapsed_ns = 20;
    totals.after.elapsed_ns = 25;
    totals.root_generation.elapsed_ns = 25;
    assert_eq!(totals.bank_interval_ns(start, end).unwrap(), 100);
    assert!(totals.bank_interval_ns(start, start).is_err());
    assert!(totals.bank_interval_ns(end, start).is_err());
    totals.root_generation.elapsed_ns = 26;
    assert!(totals.bank_interval_ns(start, end).is_err());
    totals.before.elapsed_ns = u64::MAX;
    assert!(totals.checked_sum_ns().is_err());
    let too_far = start
        .checked_add(Duration::from_secs(u64::MAX / 1_000_000_000 + 1))
        .unwrap();
    assert!(Totals::elapsed_ns(start, too_far).is_err());
}

#[test]
fn duration_production_hooks_and_armed_publication_boundaries_are_source_bound() {
    let engine = include_str!("device_gfx950_scoped_currentness_v1.rs");
    assert_eq!(engine.matches("duration::Measured::new(").count(), 4);
    assert_eq!(
        engine
            .matches("attempt.backend.validate_durations(&self.counts)?;")
            .count(),
        1
    );
    let finish = engine
        .split("fn finish<B: CheckpointBackend<Snapshot = S>>(")
        .nth(1)
        .unwrap();
    assert!(
        finish.find("validate_durations").unwrap() < finish.find("self.closed = true").unwrap()
    );
    let bank = include_str!("engineering_gfx950_peer_scoped_bank_rearm_v1.rs");
    let body = bank.split("unsafe fn rearm_bank_scoped(").nth(1).unwrap();
    assert!(body.find("committed: false").unwrap() < body.find("diagnostic_started").unwrap());
    assert!(
        body.find("bank_interval_ns(").unwrap()
            < body.find("deadline_check(backend.now(), until)?;").unwrap()
    );
    assert!(
        body.find("bank_interval_ns(").unwrap() < body.find("backend.committed = true").unwrap()
    );
    let tail = include_str!("engineering_gfx950_peer_scoped_tail_v1.rs");
    let build = tail
        .split("impl ClosedBackend for NativeTail")
        .nth(1)
        .unwrap();
    assert!(build.contains("result.currentness_durations = self"));
    assert!(build.contains(".durations()?;"));
    let layer = include_str!("engineering_gfx950_peer_scoped_layer_v1.rs");
    let commit = layer.split("fn commit(").nth(2).unwrap();
    assert!(
        commit.find(".durations()?;").unwrap() < commit.find("self.op.pair.completed =").unwrap()
    );
}
