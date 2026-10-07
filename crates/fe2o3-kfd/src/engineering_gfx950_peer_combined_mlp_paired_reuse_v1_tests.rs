use super::*;

struct Fake {
    clock: Instant,
    signals: Values,
    sealed: [(u64, u64); 2],
    observed: [(u64, u64); 2],
    writes: [u64; 2],
    completed: [u64; 2],
    expected_identity: [[u64; 8]; 2],
    actual_identity: [[u64; 8]; 2],
    events: Vec<&'static str>,
    calls: usize,
    fail: usize,
    panic: usize,
    expire: usize,
    poisoned: bool,
}

impl Fake {
    fn new() -> Self {
        Self {
            clock: Instant::now(),
            signals: [[0; PACKETS]; 2],
            sealed: [(5, 3); 2],
            observed: [(10, 7); 2],
            writes: [10; 2],
            completed: [10; 2],
            expected_identity: [[1, 2, 3, 4, 5, 6, 7, 8]; 2],
            actual_identity: [[1, 2, 3, 4, 5, 6, 7, 8]; 2],
            events: Vec::new(),
            calls: 0,
            fail: 0,
            panic: 0,
            expire: usize::MAX,
            poisoned: false,
        }
    }
    fn step(&mut self, event: &'static str) -> Result<()> {
        if self.poisoned {
            return Err("fake remains quarantined".into());
        }
        self.calls += 1;
        self.events.push(event);
        assert_ne!(self.calls, self.panic, "injected reuse unwind");
        if self.calls == self.fail {
            return Err("injected reuse failure".into());
        }
        Ok(())
    }
    fn attempt(&mut self, old: u64, next: u64, current: u64) -> Result<()> {
        let until = self.clock + Duration::from_millis(10);
        recycle(self, old, next, current, until)
    }
    fn complete_with_intervening_work(&mut self) {
        self.signals = [[0; PACKETS]; 2];
        self.sealed = self.observed;
        self.writes = self.writes.map(|v| v + 10);
        self.completed = self.writes;
        self.observed = self.writes.map(|v| (v, v - 3));
        self.calls = 0;
        self.events.clear();
    }
}

impl ReuseBackend for Fake {
    fn now(&mut self) -> Instant {
        self.clock + Duration::from_millis(if self.calls >= self.expire { 11 } else { 0 })
    }
    fn validate_old(&mut self) -> Result<()> {
        self.step("old")?;
        if self.actual_identity != self.expected_identity {
            return Err("changed allocation/queue identity".into());
        }
        retired_gate(
            self.signals,
            self.sealed,
            self.sealed,
            self.writes,
            self.completed,
            self.sealed.map(|v| v.1),
            self.observed,
        )?;
        consumed_gate(self.sealed, self.observed)
    }
    fn write_kernargs(&mut self, rank: usize) -> Result<()> {
        self.step(if rank == 0 { "kernarg0" } else { "kernarg1" })
    }
    fn reset_signal(&mut self, rank: usize, slot: usize) -> Result<()> {
        self.step("reset")?;
        if self.signals[rank][slot] != 0 {
            return Err("duplicate or premature reset".into());
        }
        self.signals[rank][slot] = 1;
        Ok(())
    }
    fn validate_new(&mut self) -> Result<()> {
        self.step("new")?;
        if self.signals != [[1; PACKETS]; 2] {
            return Err("pending readback".into());
        }
        Ok(())
    }
    fn quarantine(&mut self) {
        self.poisoned = true;
    }
}

#[test]
fn paired_reuse_checks_both_batches_before_writes_and_resets() {
    let mut fake = Fake::new();
    let reads = fake.observed.map(|v| v.1);
    fake.attempt(1, 2, 2).unwrap();
    assert_eq!(&fake.events[..4], &["old", "kernarg0", "kernarg1", "old"]);
    assert_eq!(&fake.events[4..14], &["reset"; 10]);
    assert_eq!(fake.events[14], "new");
    assert!(!fake.poisoned);
    assert_eq!(fake.observed.map(|v| v.1), reads); // No read credit is manufactured.
}

#[test]
fn paired_reuse_rejects_zero_stale_skipped_and_exhausted_generation() {
    for (old, next, current) in [
        (0, 1, 1),
        (1, 1, 1),
        (1, 3, 3),
        (1, 2, 1),
        (1, 2, 3),
        (u64::MAX, 0, 0),
        (u64::MAX, u64::MAX, u64::MAX),
    ] {
        let mut fake = Fake::new();
        assert!(fake.attempt(old, next, current).is_err());
        assert!(fake.events.is_empty());
        assert!(fake.poisoned);
    }
    let mut fake = Fake::new();
    fake.attempt(u64::MAX - 1, u64::MAX, u64::MAX).unwrap();
}

#[test]
fn paired_reuse_rejects_each_pending_peer_signal_before_any_mutation() {
    for rank in 0..2 {
        for slot in 0..PACKETS {
            for value in [-1, 1, 2, i64::MIN, i64::MAX] {
                let mut fake = Fake::new();
                fake.signals[rank][slot] = value;
                assert!(fake.attempt(1, 2, 2).is_err());
                assert_eq!(fake.events, ["old"]);
                assert!(fake.poisoned);
            }
        }
    }
}

#[test]
fn paired_reuse_requires_real_consumption_and_unchanged_identity() {
    for rank in 0..2 {
        for mutation in 0..7 {
            let mut fake = Fake::new();
            match mutation {
                0 => fake.observed[rank].1 = 3, // Retired gate allows it, reuse must not.
                1 => fake.observed[rank].1 = 11,
                2 => fake.observed[rank].0 = 11,
                3 => fake.completed[rank] = 5,
                4 => fake.writes[rank] = 4,
                5 => fake.sealed[rank].1 = 6,
                6 => fake.sealed[rank].0 = 0,
                _ => unreachable!(),
            }
            assert!(fake.attempt(1, 2, 2).is_err());
            assert_eq!(fake.events, ["old"]);
            assert!(fake.poisoned);
        }
        // The adapter is required to revalidate allocation/VA/extent/backing,
        // unique GPU, GPU handle, queue epoch and queue ID, not just a snapshot.
        for field in 0..8 {
            let mut fake = Fake::new();
            fake.actual_identity[rank][field] += 1;
            assert!(fake.attempt(1, 2, 2).is_err());
            assert_eq!(fake.events, ["old"]);
            assert!(fake.poisoned);
        }
    }
}

#[test]
fn paired_reuse_each_partial_failure_quarantines_without_retry_or_fallback() {
    for failure in 1..=15 {
        let mut fake = Fake::new();
        fake.fail = failure;
        assert!(fake.attempt(1, 2, 2).is_err());
        assert_eq!(fake.calls, failure);
        assert!(fake.poisoned);
        fake.fail = 0;
        assert!(fake.attempt(1, 2, 2).is_err());
        assert_eq!(fake.calls, failure);
    }
}

#[test]
fn paired_reuse_timeout_and_unwind_keep_terminal_quarantine() {
    for event in 0..=15 {
        let mut fake = Fake::new();
        fake.expire = event;
        assert!(fake.attempt(1, 2, 2).is_err());
        assert!(fake.poisoned);
        assert_eq!(fake.calls, event);
    }
    for event in 1..=15 {
        let mut fake = Fake::new();
        fake.panic = event;
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fake.attempt(1, 2, 2)))
                .is_err()
        );
        assert!(fake.poisoned);
        assert_eq!(fake.calls, event);
    }
}

#[test]
fn paired_reuse_transaction_stays_bounded_beyond_full_model_dispatch_count() {
    // Counted backend model, not a KFD/GPU allocation claim. Every iteration
    // executes the same production recycling transaction and frontier gates.
    // A native qualification must independently observe Group's actual census.
    let mut pairs: Vec<Option<Fake>> = (0..72).map(|_| None).collect();
    let mut generations = [0_u64; 72];
    let mut allocations = [0_usize; 2];
    let mut dispatches = 0;
    for forward in 0..2304 {
        let bank = forward % 2;
        let generation = (forward / 2 + 1) as u64;
        for layer in 0..36 {
            let index = bank * 36 + layer;
            if let Some(fake) = pairs[index].as_mut() {
                fake.complete_with_intervening_work();
                fake.attempt(generations[index], generation, generation)
                    .unwrap();
            } else {
                pairs[index] = Some(Fake::new());
                allocations = allocations.map(|n| n + 1);
            }
            generations[index] = generation;
            dispatches += 1;
            assert!(allocations.iter().all(|&n| n <= 72));
        }
        if forward >= 1 {
            assert_eq!(allocations, [72; 2]);
        }
    }
    assert_eq!(dispatches, 2304 * 36);
    assert!(dispatches > 2303 * 36);
    assert_eq!([715 + allocations[0], 711 + allocations[1]], [787, 783]);
    assert_eq!(generations, [1152; 72]);
}
