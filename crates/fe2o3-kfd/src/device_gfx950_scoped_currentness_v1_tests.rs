use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

#[cfg(feature = "engineering-currentness-duration-diagnostics")]
#[path = "device_gfx950_currentness_duration_v1_tests.rs"]
mod duration_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    root: u64,
    generation: u64,
    contents: u64,
}

struct Recording {
    count: usize,
    ids: [u64; 2],
    retained: [Snapshot; 2],
    observed: Snapshot,
    poisoned: [bool; 2],
    events: Vec<String>,
    fail_at: Option<usize>,
    panic_at: Option<usize>,
    expire_at: Option<usize>,
    now: Instant,
    until: Instant,
}

impl Recording {
    fn new() -> Self {
        let now = Instant::now();
        let snapshot = Snapshot {
            root: 3,
            generation: 7,
            contents: 9,
        };
        Self {
            count: 2,
            ids: [11, 22],
            retained: [snapshot; 2],
            observed: snapshot,
            poisoned: [false; 2],
            events: Vec::new(),
            fail_at: None,
            panic_at: None,
            expire_at: None,
            now,
            until: now.checked_add(Duration::from_secs(10)).unwrap(),
        }
    }

    fn event(&mut self, name: String) -> Result<(), DeviceBindingError> {
        let index = self.events.len();
        self.events.push(name);
        if self.expire_at == Some(index) {
            self.now = self.until;
        }
        assert_ne!(self.panic_at, Some(index), "injected checkpoint unwind");
        if self.fail_at == Some(index) {
            return Err(changed("injected checkpoint fault"));
        }
        Ok(())
    }
}

impl CheckpointBackend for Recording {
    type Snapshot = Snapshot;
    fn now(&mut self) -> Instant {
        self.now
    }
    fn participants(&self) -> usize {
        self.count
    }
    fn identity(&mut self, rank: usize) -> Result<u64, DeviceBindingError> {
        self.event(format!("identity:{rank}"))?;
        if self.poisoned[rank] {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        Ok(self.ids[rank])
    }
    fn before(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        // Fault classes below model the exact native helper, not new syscalls.
        for name in [
            "process",
            "incarnation",
            "reset",
            "kfd_sysfs",
            "render",
            "uapi",
            "drm",
            "xnack",
            "apertures",
        ] {
            self.event(format!("before:{rank}:{name}"))?;
        }
        Ok(())
    }
    fn discover(&mut self) -> Result<Snapshot, DeviceBindingError> {
        self.event("discover".into())?;
        Ok(self.observed)
    }
    fn compare_retained(
        &mut self,
        rank: usize,
        value: &Snapshot,
    ) -> Result<(), DeviceBindingError> {
        self.event(format!("compare:{rank}"))?;
        if *value != self.retained[rank] {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }
        Ok(())
    }
    fn after(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        for name in ["kfd_fd", "render", "incarnation", "xnack", "drm", "reset"] {
            self.event(format!("after:{rank}:{name}"))?;
        }
        Ok(())
    }
    fn root_generation(&mut self, snapshot: &Snapshot) -> Result<(), DeviceBindingError> {
        self.event("root_generation".into())?;
        if (snapshot.root, snapshot.generation) != (self.observed.root, self.observed.generation) {
            return Err(changed("root or generation drift"));
        }
        Ok(())
    }
    fn poison_all(&mut self) {
        self.events.push("poison_all".into());
        self.poisoned.fill(true);
    }
}

fn exercise(backend: &mut Recording) -> Result<ScopedCountsV1, DeviceBindingError> {
    let until = backend.until;
    let mut window = Window::enter(backend, until)?;
    window.checkpoint(backend)?;
    window.checkpoint(backend)?;
    window.finish(backend)
}

#[test]
fn scoped_checkpoint_keeps_local_checks_between_two_root_probes_without_discovery() {
    let mut backend = Recording::new();
    let until = backend.until;
    let mut window = Window::enter(&mut backend, until).unwrap();
    backend.events.clear();
    window.checkpoint(&mut backend).unwrap();
    let mut expected = vec![
        "identity:0".to_owned(),
        "identity:1".to_owned(),
        "root_generation".to_owned(),
    ];
    for rank in 0..2 {
        expected.extend(
            [
                "process",
                "incarnation",
                "reset",
                "kfd_sysfs",
                "render",
                "uapi",
                "drm",
                "xnack",
                "apertures",
            ]
            .map(|name| format!("before:{rank}:{name}")),
        );
    }
    for rank in 0..2 {
        expected.extend(
            ["kfd_fd", "render", "incarnation", "xnack", "drm", "reset"]
                .map(|name| format!("after:{rank}:{name}")),
        );
    }
    expected.extend(["root_generation", "identity:0", "identity:1"].map(str::to_owned));
    assert_eq!(backend.events, expected);
    let counts = window.finish(&mut backend).unwrap();
    assert_eq!(
        counts,
        ScopedCountsV1 {
            full_discoveries: 2,
            local_checkpoints: 1,
            before_calls: 6,
            after_calls: 6,
            generation_probes: 5,
        }
    );
}

#[test]
fn scoped_entry_and_exit_each_discover_and_compare_both_retained_participants() {
    let mut backend = Recording::new();
    let counts = exercise(&mut backend).unwrap();
    assert_eq!(
        counts,
        ScopedCountsV1 {
            full_discoveries: 2,
            local_checkpoints: 2,
            before_calls: 8,
            after_calls: 8,
            generation_probes: 7,
        }
    );
    for name in ["discover", "compare:0", "compare:1"] {
        assert_eq!(
            backend
                .events
                .iter()
                .filter(|value| value.as_str() == name)
                .count(),
            2
        );
    }
    assert_eq!(backend.poisoned, [false; 2]);
}

#[test]
fn every_scoped_engine_fault_poisons_both_and_stops_at_the_original_boundary() {
    let mut control = Recording::new();
    exercise(&mut control).unwrap();
    for index in 0..control.events.len() {
        let mut backend = Recording::new();
        backend.fail_at = Some(index);
        assert!(exercise(&mut backend).is_err(), "fault {index}");
        assert_eq!(&backend.events[..=index], &control.events[..=index]);
        assert_eq!(backend.events.len(), index + 2);
        assert_eq!(
            backend.events.last().map(String::as_str),
            Some("poison_all")
        );
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn every_scoped_engine_unwind_poisons_both_without_later_observation() {
    let mut control = Recording::new();
    exercise(&mut control).unwrap();
    for index in 0..control.events.len() {
        let mut backend = Recording::new();
        backend.panic_at = Some(index);
        assert!(catch_unwind(AssertUnwindSafe(|| exercise(&mut backend))).is_err());
        assert_eq!(&backend.events[..=index], &control.events[..=index]);
        assert_eq!(backend.events.len(), index + 2);
        assert_eq!(
            backend.events.last().map(String::as_str),
            Some("poison_all")
        );
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn scoped_deadlines_refuse_entry_and_expiry_after_every_observed_effect() {
    let mut control = Recording::new();
    exercise(&mut control).unwrap();
    let mut expired = Recording::new();
    expired.now = expired.until;
    assert!(exercise(&mut expired).is_err());
    assert_eq!(expired.events, ["poison_all"]);
    for index in 0..control.events.len() {
        let mut backend = Recording::new();
        backend.expire_at = Some(index);
        assert!(exercise(&mut backend).is_err(), "expiry {index}");
        assert_eq!(backend.poisoned, [true; 2]);
        assert_eq!(
            backend.events.last().map(String::as_str),
            Some("poison_all")
        );
    }
}

#[test]
fn scoped_roster_count_duplicate_zero_and_substitution_are_terminal() {
    for count in [0, 1, 3, 8] {
        let mut backend = Recording::new();
        backend.count = count;
        assert!(exercise(&mut backend).is_err());
        assert_eq!(backend.events, ["poison_all"]);
    }
    for ids in [[0, 22], [11, 0], [11, 11]] {
        let mut backend = Recording::new();
        backend.ids = ids;
        assert!(exercise(&mut backend).is_err());
        assert_eq!(backend.poisoned, [true; 2]);
    }
    for ids in [[22, 11], [11, 33]] {
        let mut backend = Recording::new();
        let until = backend.until;
        let mut window = Window::enter(&mut backend, until).unwrap();
        backend.ids = ids;
        backend.events.clear();
        assert!(window.checkpoint(&mut backend).is_err());
        assert!(
            !backend
                .events
                .iter()
                .any(|name| name.starts_with("before:"))
        );
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn scoped_root_or_generation_drift_rejects_before_local_predicates() {
    for root in [false, true] {
        let mut backend = Recording::new();
        let until = backend.until;
        let mut window = Window::enter(&mut backend, until).unwrap();
        if root {
            backend.observed.root += 1;
        } else {
            backend.observed.generation += 1;
        }
        backend.events.clear();
        assert!(window.checkpoint(&mut backend).is_err());
        assert!(
            !backend
                .events
                .iter()
                .any(|name| name.starts_with("before:"))
        );
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn scoped_persistent_content_change_without_generation_is_caught_at_full_exit() {
    let mut backend = Recording::new();
    let until = backend.until;
    let mut window = Window::enter(&mut backend, until).unwrap();
    backend.observed.contents += 1;
    window.checkpoint(&mut backend).unwrap();
    assert!(window.finish(&mut backend).is_err());
    assert_eq!(backend.poisoned, [true; 2]);
}

#[test]
fn scoped_transient_content_change_and_revert_is_an_explicit_temporal_limitation() {
    let mut backend = Recording::new();
    let until = backend.until;
    let mut window = Window::enter(&mut backend, until).unwrap();
    let original = backend.observed;
    backend.observed.contents += 1;
    window.checkpoint(&mut backend).unwrap();
    backend.observed = original;
    assert!(window.finish(&mut backend).is_ok());
    // Success here demonstrates non-equivalence, not a proof of unchanged facts.
    assert_eq!(backend.poisoned, [false; 2]);
}

#[test]
fn scoped_exit_is_bound_to_entry_even_if_retained_snapshots_are_replaced() {
    let mut backend = Recording::new();
    let until = backend.until;
    let mut window = Window::enter(&mut backend, until).unwrap();
    backend.observed.contents += 1;
    backend.retained = [backend.observed; 2];
    assert!(window.finish(&mut backend).is_err());
    assert_eq!(backend.poisoned, [true; 2]);
}

#[test]
fn scoped_closed_or_poisoned_windows_never_resume() {
    for close in [false, true] {
        let mut backend = Recording::new();
        let until = backend.until;
        let mut window = Window::enter(&mut backend, until).unwrap();
        if close {
            window.finish(&mut backend).unwrap();
        } else {
            backend.fail_at = Some(backend.events.len());
            assert!(window.checkpoint(&mut backend).is_err());
            backend.fail_at = None;
        }
        backend.events.clear();
        assert!(window.checkpoint(&mut backend).is_err());
        assert_eq!(backend.events, ["poison_all"]);
        assert!(window.finish(&mut backend).is_err());
        assert_eq!(backend.poisoned, [true; 2]);
    }
}

#[test]
fn scoped_counter_overflow_is_terminal_before_unaccounted_work() {
    let mut backend = Recording::new();
    let until = backend.until;
    let mut window = Window::enter(&mut backend, until).unwrap();
    window.counts.generation_probes = u64::MAX;
    backend.events.clear();
    assert!(window.checkpoint(&mut backend).is_err());
    assert_eq!(backend.events, ["identity:0", "identity:1", "poison_all"]);
    assert_eq!(backend.poisoned, [true; 2]);
}

struct One<'a> {
    backend: &'a mut Recording,
    physical_rank: usize,
}
impl CheckpointBackend for One<'_> {
    type Snapshot = Snapshot;
    fn now(&mut self) -> Instant {
        self.backend.now()
    }
    fn participants(&self) -> usize {
        1
    }
    fn identity(&mut self, rank: usize) -> Result<u64, DeviceBindingError> {
        assert_eq!(rank, 0);
        self.backend.identity(self.physical_rank)
    }
    fn before(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        assert_eq!(rank, 0);
        self.backend.before(self.physical_rank)
    }
    fn after(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        assert_eq!(rank, 0);
        self.backend.after(self.physical_rank)
    }
    fn discover(&mut self) -> Result<Snapshot, DeviceBindingError> {
        panic!("a rank checkpoint must not rediscover topology")
    }
    fn compare_retained(&mut self, _: usize, _: &Snapshot) -> Result<(), DeviceBindingError> {
        panic!("a rank checkpoint cannot claim a full retained comparison")
    }
    fn root_generation(&mut self, value: &Snapshot) -> Result<(), DeviceBindingError> {
        self.backend.root_generation(value)
    }
    fn poison_all(&mut self) {
        self.backend.events.push("poison_selected".into());
        self.backend.poisoned[self.physical_rank] = true;
    }
}
#[test]
fn scoped_rank_checkpoint_preserves_exact_one_rank_predicates_and_full_exit() {
    for rank in 0..2 {
        let mut backend = Recording::new();
        let until = backend.until;
        let mut window = Window::enter(&mut backend, until).unwrap();
        backend.events.clear();
        window
            .checkpoint_rank(
                &mut One {
                    backend: &mut backend,
                    physical_rank: rank,
                },
                rank,
            )
            .unwrap();
        assert_eq!(backend.events.first(), Some(&format!("identity:{rank}")));
        assert_eq!(backend.events.last(), Some(&format!("identity:{rank}")));
        assert_eq!(
            backend
                .events
                .iter()
                .filter(|e| e.as_str() == "root_generation")
                .count(),
            2
        );
        assert!(
            !backend
                .events
                .iter()
                .any(|e| e.contains(&format!(":{}", 1 - rank)))
        );
        let counts = window.finish(&mut backend).unwrap();
        assert_eq!(counts.full_discoveries, 2);
        assert_eq!(counts.local_checkpoints, 1);
        assert_eq!(
            (
                counts.before_calls,
                counts.after_calls,
                counts.generation_probes
            ),
            (5, 5, 5)
        );
    }
}
#[test]
fn scoped_rank_substitution_and_out_of_range_poison_window_and_selected_device() {
    for rank in [1, 2, usize::MAX] {
        let mut backend = Recording::new();
        let until = backend.until;
        let mut window = Window::enter(&mut backend, until).unwrap();
        backend.events.clear();
        assert!(
            window
                .checkpoint_rank(
                    &mut One {
                        backend: &mut backend,
                        physical_rank: 0
                    },
                    rank
                )
                .is_err()
        );
        assert!(window.poisoned);
        assert_eq!(backend.poisoned, [true, false]);
        assert!(!backend.events.iter().any(|e| e.starts_with("before")));
    }
}
#[test]
fn scoped_rank_every_fault_and_unwind_stops_and_invalidates_whole_window() {
    let mut control = Recording::new();
    let until = control.until;
    let mut window = Window::enter(&mut control, until).unwrap();
    control.events.clear();
    window
        .checkpoint_rank(
            &mut One {
                backend: &mut control,
                physical_rank: 1,
            },
            1,
        )
        .unwrap();
    for unwind in [false, true] {
        for index in 0..control.events.len() {
            let mut backend = Recording::new();
            let until = backend.until;
            let mut window = Window::enter(&mut backend, until).unwrap();
            backend.events.clear();
            if unwind {
                backend.panic_at = Some(index);
            } else {
                backend.fail_at = Some(index);
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                window.checkpoint_rank(
                    &mut One {
                        backend: &mut backend,
                        physical_rank: 1,
                    },
                    1,
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(&backend.events[..=index], &control.events[..=index]);
            assert_eq!(backend.events.len(), index + 2);
            assert!(window.poisoned);
            assert_eq!(backend.poisoned, [false, true]);
        }
    }
}
#[test]
fn scoped_rank_deadline_and_generation_drift_are_terminal_before_later_effects() {
    for expire in [false, true] {
        let mut backend = Recording::new();
        let until = backend.until;
        let mut window = Window::enter(&mut backend, until).unwrap();
        backend.events.clear();
        if expire {
            backend.now = until;
        } else {
            backend.observed.generation += 1;
        }
        assert!(
            window
                .checkpoint_rank(
                    &mut One {
                        backend: &mut backend,
                        physical_rank: 0
                    },
                    0
                )
                .is_err()
        );
        assert!(window.poisoned);
        assert!(!backend.events.iter().any(|e| e.starts_with("before")));
        assert_eq!(backend.poisoned, [true, false]);
    }
}
