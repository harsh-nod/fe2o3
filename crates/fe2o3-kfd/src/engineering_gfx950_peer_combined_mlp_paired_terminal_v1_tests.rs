use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn state() -> CombinedMlpSnapshotV1 {
    let mut prefix = [0; PREFIX_WORDS];
    prefix[0] = 1;
    prefix[3] = 31;
    prefix[4..9].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[9..14].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[14..22].fill(u32::MAX);
    prefix[22] = 3;
    prefix[23..31].fill(u32::MAX);
    prefix[31] = 3;
    prefix[32..].fill(64);
    CombinedMlpSnapshotV1 {
        prefix,
        guard: guard_words(1, 1),
    }
}

struct Fake {
    clock: Instant,
    calls: usize,
    events: Vec<String>,
    fail: usize,
    panic: usize,
    expire: usize,
    after: bool,
    owners: [Activation; 2],
    generations: [u64; 2],
    identity: [bool; 2],
    retired: [bool; 2],
    states: [CombinedMlpSnapshotV1; 2],
    observations: [usize; 2],
    drift: Option<(usize, usize)>,
    signals: [[i64; 5]; 2],
    frontiers: [(u64, u64); 2],
    previous: [u64; 2],
    queue_fault: i64,
    full_fences: usize,
    invalid_full_fence: usize,
    group_poisoned: bool,
    staged_poisoned: bool,
    poisoned: usize,
    commit_panic: bool,
}

impl Fake {
    fn new() -> Self {
        Self {
            clock: Instant::now(),
            calls: 0,
            events: Vec::new(),
            fail: 0,
            panic: 0,
            expire: 0,
            after: false,
            owners: [Activation::Submitted; 2],
            generations: [1; 2],
            identity: [true; 2],
            retired: [true; 2],
            states: [state(), state()],
            observations: [0; 2],
            drift: None,
            signals: [[0; 5]; 2],
            frontiers: [(5, 0); 2],
            previous: [0; 2],
            queue_fault: 0,
            full_fences: 0,
            invalid_full_fence: 0,
            group_poisoned: false,
            staged_poisoned: false,
            poisoned: 0,
            commit_panic: false,
        }
    }
    fn step(&mut self, event: String, effect: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        self.calls += 1;
        self.events.push(event);
        assert_eq!(self.owners, [Activation::Submitted; 2]);
        if self.panic == self.calls && !self.after {
            panic!("before effect");
        }
        if self.fail == self.calls && !self.after {
            return Err("before effect".into());
        }
        effect(self)?;
        if self.expire == self.calls {
            self.clock += Duration::from_millis(10);
        }
        if self.panic == self.calls && self.after {
            panic!("after effect");
        }
        if self.fail == self.calls && self.after {
            return Err("after effect".into());
        }
        Ok(())
    }
    fn queue_state(&mut self) -> Result<()> {
        if self.queue_fault != 0 {
            return Err("queue exception".into());
        }
        for rank in 0..2 {
            require_completed_frontier(5, 5)?;
            validate_counters(5, self.previous[rank], self.frontiers[rank])?;
            self.previous[rank] = self.frontiers[rank].1;
        }
        Ok(())
    }
    fn quarantined(&self) {
        assert_eq!(self.owners, [Activation::Poisoned; 2]);
        assert!(self.group_poisoned && self.staged_poisoned);
        assert_eq!(self.poisoned, 1);
    }
    fn run(&mut self) -> Result<([CombinedMlpSnapshotV1; 2], [(u64, u64); 2])> {
        let deadline = self.clock + Duration::from_millis(10);
        run(self, deadline)
    }
}

impl TerminalBackend for Fake {
    fn now(&mut self) -> Instant {
        self.clock
    }
    fn generation(&self) -> u64 {
        1
    }
    fn custody(&mut self) -> Result<()> {
        self.step("custody".into(), |s| {
            if s.retired != [true; 2] || s.identity != [true; 2] || s.generations != [1; 2] {
                return Err("custody".into());
            }
            Ok(())
        })
    }
    fn full_fence(&mut self) -> Result<()> {
        self.step("full".into(), |s| {
            s.full_fences += 1;
            if s.full_fences == s.invalid_full_fence {
                return Err("topology/aperture/reset".into());
            }
            s.queue_state()
        })
    }
    fn queues(&mut self) -> Result<()> {
        self.step("queues".into(), Self::queue_state)
    }
    fn observe(&mut self, rank: usize) -> Result<CombinedMlpSnapshotV1> {
        self.step(format!("observe{rank}"), |s| {
            s.observations[rank] += 1;
            if !s.identity[rank] || s.generations[rank] != 1 {
                return Err("owner token/generation".into());
            }
            Ok(())
        })?;
        let mut value = self.states[rank].clone();
        if self.drift == Some((rank, self.observations[rank])) {
            value.prefix[32] = 63;
        }
        Ok(value)
    }
    fn signals(&mut self, _deadline: Instant) -> Result<[(u64, u64); 2]> {
        self.step("signals".into(), |s| {
            if !arena::completion_gate(s.signals, [true; 2], s.frontiers, [5; 2], s.previous)? {
                return Err("incomplete signals".into());
            }
            Ok(())
        })?;
        Ok(self.frontiers)
    }
    fn commit(&mut self) -> Result<()> {
        self.step("commit".into(), |s| {
            assert_eq!(s.full_fences, 2);
            assert_eq!(s.observations, [3; 2]);
            s.owners[0] = Activation::Completed;
            assert!(!s.commit_panic, "between owner transitions");
            s.owners[1] = Activation::Completed;
            Ok(())
        })
    }
    fn quarantine(&mut self) {
        self.owners.fill(Activation::Poisoned);
        self.group_poisoned = true;
        self.staged_poisoned = true;
        self.poisoned += 1;
    }
}

#[test]
fn terminal_pair_keeps_six_queue_probes_and_three_snapshots_until_full_exit() {
    let mut fake = Fake::new();
    let (states, frontiers) = fake.run().unwrap();
    assert_eq!(
        fake.events,
        [
            "custody", "full", "observe0", "observe1", "queues", "queues", "observe0", "queues",
            "queues", "observe1", "queues", "queues", "signals", "observe0", "observe1", "full",
            "commit"
        ]
    );
    assert_eq!(states, [state(), state()]);
    assert_eq!(frontiers, [(5, 0); 2]);
    assert_eq!(fake.owners, [Activation::Completed; 2]);
    assert_eq!(fake.poisoned, 0);
}

#[test]
fn terminal_pair_every_failure_before_and_after_effect_quarantines_both() {
    let mut success = Fake::new();
    success.run().unwrap();
    for fail in 1..=success.calls {
        for after in [false, true] {
            let mut fake = Fake::new();
            fake.fail = fail;
            fake.after = after;
            assert!(fake.run().is_err(), "{fail}/{after}");
            assert_eq!(fake.calls, fail);
            fake.quarantined();
        }
    }
}

#[test]
fn terminal_pair_every_unwind_and_partial_commit_quarantines_both() {
    let mut success = Fake::new();
    success.run().unwrap();
    for panic in 1..=success.calls {
        for after in [false, true] {
            let mut fake = Fake::new();
            fake.panic = panic;
            fake.after = after;
            assert!(catch_unwind(AssertUnwindSafe(|| fake.run())).is_err());
            assert_eq!(fake.calls, panic);
            fake.quarantined();
        }
    }
    let mut fake = Fake::new();
    fake.commit_panic = true;
    assert!(catch_unwind(AssertUnwindSafe(|| fake.run())).is_err());
    fake.quarantined();
}

#[test]
fn terminal_pair_every_deadline_including_commit_adjacent_refuses_publication() {
    let mut success = Fake::new();
    success.run().unwrap();
    for expire in 1..=success.calls {
        let mut fake = Fake::new();
        fake.expire = expire;
        assert!(fake.run().is_err(), "{expire}");
        assert_eq!(fake.calls, expire);
        fake.quarantined();
    }
    let mut fake = Fake::new();
    let deadline = fake.clock;
    assert!(run(&mut fake, deadline).is_err());
    assert_eq!(fake.calls, 0);
    fake.quarantined();
}

#[test]
fn terminal_pair_rank_one_identity_retirement_generation_and_guard_are_required() {
    for rank in 0..2 {
        for defect in 0..6 {
            let mut fake = Fake::new();
            match defect {
                0 => fake.identity[rank] = false,
                1 => fake.retired[rank] = false,
                2 => fake.generations[rank] = 2,
                3 => fake.states[rank].guard[0] = 2,
                4 => fake.states[rank].guard[2] = 0,
                _ => fake.states[rank].prefix[547] = 0,
            }
            assert!(fake.run().is_err());
            fake.quarantined();
            assert!(!fake.events.iter().any(|s| s == "commit"));
        }
    }
}

#[test]
fn terminal_pair_second_and_final_atomic_readback_drift_refuse() {
    for rank in 0..2 {
        for observation in [2, 3] {
            let mut fake = Fake::new();
            fake.drift = Some((rank, observation));
            assert!(fake.run().is_err());
            fake.quarantined();
            assert!(!fake.events.iter().any(|s| s == "commit"));
        }
    }
}

#[test]
fn terminal_pair_both_full_fences_and_queue_errors_are_mandatory() {
    for fence in [1, 2] {
        let mut fake = Fake::new();
        fake.invalid_full_fence = fence;
        assert!(fake.run().is_err());
        fake.quarantined();
        assert!(!fake.events.iter().any(|s| s == "commit"));
    }
    for rank in 0..2 {
        for defect in 0..4 {
            let mut fake = Fake::new();
            match defect {
                0 => fake.frontiers[rank].0 = 4,
                1 => fake.frontiers[rank].1 = 6,
                2 => fake.previous[rank] = 1,
                _ => fake.queue_fault = 1,
            }
            assert!(fake.run().is_err());
            fake.quarantined();
        }
    }
}

#[test]
fn terminal_pair_all_ten_actual_signals_and_lagging_read_credit_are_preserved() {
    for rank in 0..2 {
        for slot in 0..5 {
            for value in [1, -1, 2] {
                let mut fake = Fake::new();
                fake.signals[rank][slot] = value;
                assert!(fake.run().is_err());
                fake.quarantined();
                assert!(!fake.events.iter().any(|s| s == "commit"));
            }
        }
    }
    for read in [0, 1, 4, 5] {
        let mut fake = Fake::new();
        fake.frontiers = [(5, read); 2];
        assert_eq!(fake.run().unwrap().1, [(5, read); 2]);
        assert_eq!(fake.previous, [read; 2]);
    }
}

#[test]
fn terminal_pair_public_signature_keeps_borrow_free_pair_and_short_group_borrow() {
    use crate::{
        Gfx950EngineeringPeerGroupV1 as Group, Gfx950EngineeringPeerGuardedMlpInputsV1 as Inputs,
        Gfx950EngineeringPeerGuardedMlpObservationV1 as Observation,
        Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair,
    };
    let _: for<'g, 'p, 'k> fn(
        &'g mut Group,
        &'p mut Pair,
        Inputs<'k>,
        u32,
    ) -> std::result::Result<Observation, String> =
        Group::dispatch_guarded_mlp_pair_paired_terminal_v1;
}
