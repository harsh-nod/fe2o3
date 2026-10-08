use super::*;
use layer_duration::tests::ScriptClock;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn paired_duration_exact_seven_stages_preserve_original_effect_trace() {
    let mut old = Fake::new();
    let before = old.clock;
    let original = coordinate(&mut old, 5).unwrap();
    let mut measured = Fake::new();
    measured.clock = before;
    let mut clock = ScriptClock::new(8);
    let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
    let actual = coordinate_recorded(
        &mut measured,
        5,
        None,
        TerminalPolicy::Legacy,
        Some(&mut recording),
    )
    .unwrap();
    assert_eq!(recording.finish().unwrap(), ([10; 7], 70));
    assert_eq!(actual.states, original.states);
    assert_eq!(
        actual.observed_queue_frontiers,
        original.observed_queue_frontiers
    );
    assert_eq!(actual.segment_host_ns, original.segment_host_ns);
    assert_eq!(measured.events, old.events);
    assert_eq!(measured.clock, old.clock);
    assert_eq!(measured.polls, old.polls);
    assert_eq!(clock.index, 8);
}

#[test]
fn paired_duration_every_clock_error_is_terminal() {
    for boundary in 0..8 {
        let mut backend = Fake::new();
        let mut clock = ScriptClock::new(8);
        clock.fail = Some(boundary);
        let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
        assert!(
            coordinate_recorded(
                &mut backend,
                5,
                None,
                TerminalPolicy::Legacy,
                Some(&mut recording)
            )
            .is_err()
        );
        assert_eq!(backend.poisoned, [true; 4]);
        assert_eq!(backend.poison_calls, 1);
        assert_eq!(backend.events.last().unwrap(), "poison");
    }
}

#[test]
fn paired_duration_late_backward_and_overflow_are_terminal() {
    for offset in [
        Duration::ZERO,
        Duration::from_secs(u64::MAX / 1_000_000_000 + 1),
    ] {
        let mut backend = Fake::new();
        let mut clock = ScriptClock::new(8);
        clock.offsets[7] = offset;
        let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
        assert!(
            coordinate_recorded(
                &mut backend,
                5,
                None,
                TerminalPolicy::Legacy,
                Some(&mut recording)
            )
            .is_err()
        );
        assert!(backend.terminal_validated);
        assert_eq!(backend.poisoned, [true; 4]);
        assert_eq!(backend.poison_calls, 1);
    }
}

#[test]
fn paired_duration_preserves_all_effect_and_deadline_refusals() {
    let mut count = Fake::new();
    coordinate(&mut count, 5).unwrap();
    for operation in 1..=count.operations {
        for after_effect in [false, true] {
            let mut old = Fake::new();
            old.fail_at = operation;
            old.after_effect = after_effect;
            assert!(coordinate(&mut old, 5).is_err());
            let mut measured = Fake::new();
            measured.fail_at = operation;
            measured.after_effect = after_effect;
            let mut clock = ScriptClock::new(8);
            let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
            assert!(
                coordinate_recorded(
                    &mut measured,
                    5,
                    None,
                    TerminalPolicy::Legacy,
                    Some(&mut recording)
                )
                .is_err()
            );
            assert_eq!(measured.events, old.events);
            assert_eq!(measured.poisoned, old.poisoned);
        }
        let mut old = Fake::new();
        old.expire_at = operation;
        assert!(coordinate(&mut old, 5).is_err());
        let mut measured = Fake::new();
        measured.expire_at = operation;
        let mut clock = ScriptClock::new(8);
        let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
        assert!(
            coordinate_recorded(
                &mut measured,
                5,
                None,
                TerminalPolicy::Legacy,
                Some(&mut recording)
            )
            .is_err()
        );
        assert_eq!(measured.events, old.events);
        assert_eq!(measured.poisoned, old.poisoned);
    }
}

#[test]
fn paired_duration_unwind_remains_inside_closed_owner_custody() {
    struct Owner<'a> {
        backend: &'a mut Fake,
        committed: bool,
    }
    impl Drop for Owner<'_> {
        fn drop(&mut self) {
            if !self.committed {
                self.backend.poison();
            }
        }
    }
    for boundary in 0..8 {
        let mut backend = Fake::new();
        let mut clock = ScriptClock::new(8);
        clock.unwind = Some(boundary);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut owner = Owner {
                backend: &mut backend,
                committed: false,
            };
            let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
            let result = coordinate_recorded(
                owner.backend,
                5,
                None,
                TerminalPolicy::Legacy,
                Some(&mut recording),
            );
            owner.committed = result.is_ok();
            result
        }));
        assert!(outcome.is_err());
        assert_eq!(backend.poisoned, [true; 4]);
        assert_eq!(backend.poison_calls, 1);
    }
}

#[test]
fn paired_duration_ordinary_call_has_no_diagnostic_clock() {
    let mut backend = Fake::new();
    let clock = ScriptClock::new(0);
    coordinate_recorded(&mut backend, 5, None, TerminalPolicy::Legacy, None).unwrap();
    assert_eq!(clock.index, 0);
    assert_eq!(backend.poison_calls, 0);
    let native = include_str!("engineering_gfx950_peer_scoped_layer_v1.rs");
    assert!(native.contains("Some(&mut recording)"));
    let coordinator = include_str!("engineering_gfx950_peer_combined_mlp_paired_v1.rs");
    let wrapper = coordinator
        .split("fn coordinate_with_terminal_policy(")
        .nth(1)
        .unwrap()
        .split("fn coordinate_recorded(")
        .next()
        .unwrap();
    assert!(wrapper.contains("None,"));
    assert!(!wrapper.contains("Monotonic"));
}

#[test]
fn paired_duration_marks_follow_exact_complete_stage_boundaries() {
    use std::cell::Cell;
    use std::rc::Rc;
    struct Counted {
        inner: Fake,
        count: Rc<Cell<usize>>,
    }
    impl Counted {
        fn call<T>(&mut self, f: impl FnOnce(&mut Fake) -> Result<T>) -> Result<T> {
            let result = f(&mut self.inner);
            self.count.set(self.inner.operations);
            result
        }
    }
    impl CoordinatorBackend for Counted {
        fn now(&mut self) -> Instant {
            self.inner.now()
        }
        fn preflight(&mut self) -> Result<()> {
            self.call(|v| v.preflight())
        }
        fn consume(&mut self, rank: usize) -> Result<()> {
            self.call(|v| v.consume(rank))
        }
        fn reserve(&mut self, rank: usize) -> Result<()> {
            self.call(|v| v.reserve(rank))
        }
        fn fence(&mut self) -> Result<()> {
            self.call(|v| v.fence())
        }
        fn publish(&mut self, rank: usize, until: Instant) -> Result<()> {
            self.call(|v| v.publish(rank, until))
        }
        fn poll(&mut self, until: Instant) -> Result<bool> {
            self.call(|v| v.poll(until))
        }
        fn retire(&mut self, rank: usize, until: Instant) -> Result<()> {
            self.call(|v| v.retire(rank, until))
        }
        fn terminal(&mut self) -> Result<()> {
            self.call(|v| v.terminal())
        }
        fn complete(&mut self, rank: usize) -> Result<CombinedMlpSnapshotV1> {
            self.call(|v| v.complete(rank))
        }
        fn finish(
            &mut self,
            states: &[CombinedMlpSnapshotV1; 2],
            until: Instant,
        ) -> Result<[(u64, u64); 2]> {
            self.call(|v| v.finish(states, until))
        }
        fn pause(&mut self) {
            self.inner.pause();
        }
        fn poison(&mut self) {
            self.inner.poison();
        }
    }
    struct ObservingClock {
        clock: ScriptClock,
        count: Rc<Cell<usize>>,
        seen: Vec<usize>,
    }
    impl layer_duration::Clock for ObservingClock {
        fn now(&mut self) -> Result<Instant> {
            self.seen.push(self.count.get());
            layer_duration::Clock::now(&mut self.clock)
        }
    }
    let count = Rc::new(Cell::new(0));
    let mut backend = Counted {
        inner: Fake::new(),
        count: Rc::clone(&count),
    };
    let mut clock = ObservingClock {
        clock: ScriptClock::new(8),
        count,
        seen: vec![],
    };
    let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
    coordinate_recorded(
        &mut backend,
        5,
        None,
        TerminalPolicy::Legacy,
        Some(&mut recording),
    )
    .unwrap();
    assert_eq!(recording.finish().unwrap(), ([10; 7], 70));
    assert_eq!(clock.seen, [0, 1, 3, 5, 9, 13, 15, 19]);
    assert_eq!(
        backend
            .inner
            .events
            .iter()
            .filter(|v| v.as_str() == "pause")
            .count(),
        1
    );
}
