use super::super::scoped;
use super::*;

#[test]
fn scoped_bank_facade_requires_concrete_exclusive_typed_roster() {
    let _entry: for<'group, 'entries, 'owners> unsafe fn(
        &'group mut Gfx950EngineeringPeerGroupV1,
        &'entries mut [Entry<'owners>],
        u32,
    ) -> Result<
        Gfx950EngineeringPeerScopedBankRearmObservationV1,
    > = Gfx950EngineeringPeerGroupV1::rearm_guarded_mlp_bank_scoped_currentness_unchecked_v1;
}

// The qualified fake owns each effect and its before/after fault points.
// This adapter drives the same whole-roster transact engine as ScopedNative;
// device checkpoint internals retain their separate qualified fault tests.
struct ScopedFake {
    inner: Fake,
    boundary: scoped::Boundary,
    local: u64,
}
impl ScopedFake {
    fn new() -> Self {
        Self {
            inner: Fake::new(36, Mode::Rearm),
            boundary: scoped::Boundary::Entry,
            local: 0,
        }
    }
    fn counts(&self) -> crate::device::ScopedCountsV1 {
        crate::device::ScopedCountsV1 {
            full_discoveries: 2,
            local_checkpoints: self.local,
            before_calls: 4 + 2 * self.local,
            after_calls: 4 + 2 * self.local,
            generation_probes: 3 + 2 * self.local,
        }
    }
}
impl BankBackend for ScopedFake {
    fn now(&mut self) -> Instant {
        self.inner.now()
    }
    fn enter(&mut self, mode: Mode) -> Result<()> {
        for generation in &self.inner.generations {
            scoped::admission(
                36,
                mode,
                Phase::Completed,
                ArenaPolicy::ReuseRetired,
                profiles::OutputPolicy::ExactOwnResidual,
                *generation,
                Some(*generation),
                false,
            )?;
        }
        self.inner.enter(mode)
    }
    fn fence(&mut self) -> Result<()> {
        let next = scoped::next_boundary(self.boundary)?;
        self.inner.fence()?;
        match self.boundary {
            scoped::Boundary::Entry => assert_eq!(self.local, 0),
            scoped::Boundary::BeforeReset => self.local += 1,
            scoped::Boundary::Exit => {
                scoped::checked_counts(self.counts())?;
            }
            scoped::Boundary::Closed => unreachable!(),
        }
        self.boundary = next;
        Ok(())
    }
    fn identity(&mut self, index: usize, mode: Mode) -> Result<()> {
        self.inner.identity(index, mode)
    }
    fn prefix(&mut self, index: usize, rank: usize, mode: Mode) -> Result<()> {
        self.inner.prefix(index, rank, mode)
    }
    fn pair(&mut self, index: usize, mode: Mode, until: Instant) -> Result<u64> {
        let generation = self.inner.pair(index, mode, until)?;
        self.local += 2;
        Ok(generation)
    }
    fn reset_prefix(&mut self, index: usize, rank: usize) -> Result<()> {
        self.inner.reset_prefix(index, rank)?;
        self.local += 2;
        Ok(())
    }
    fn reset_pair(&mut self, index: usize, rank: usize, next: u64) -> Result<()> {
        self.inner.reset_pair(index, rank, next)?;
        self.local += 2;
        Ok(())
    }
    fn initial(&mut self, index: usize, next: u64) -> Result<()> {
        self.inner.initial(index, next)
    }
    fn commit(&mut self, mode: Mode, next: u64) -> Result<()> {
        assert_eq!(self.boundary, scoped::Boundary::Closed);
        scoped::checked_counts(self.counts())?;
        self.inner.commit(mode, next)
    }
    fn quarantine(&mut self) {
        self.inner.quarantine();
    }
}

#[test]
fn scoped_bank_admission_requires_fixed_completed_reusable_exact_roster() {
    let valid = (
        36,
        Mode::Rearm,
        Phase::Completed,
        ArenaPolicy::ReuseRetired,
        profiles::OutputPolicy::ExactOwnResidual,
        1,
        Some(1),
        false,
    );
    let check = |v| {
        let (count, mode, phase, arena, output, generation, proof, reuse) = v;
        scoped::admission(count, mode, phase, arena, output, generation, proof, reuse)
    };
    assert!(check(valid).is_ok());
    for field in 0..12 {
        let mut value = valid;
        match field {
            0 => value.0 = 35,
            1 => value.0 = 37,
            2 => value.1 = Mode::Initial,
            3 => value.2 = Phase::Ready,
            4 => value.2 = Phase::Busy,
            5 => value.3 = ArenaPolicy::Fresh,
            6 => value.4 = profiles::OutputPolicy::Strict,
            7 => {
                value.5 = 0;
                value.6 = Some(0);
            }
            8 => {
                value.5 = u64::MAX;
                value.6 = Some(u64::MAX);
            }
            9 => value.6 = None,
            10 => value.6 = Some(2),
            _ => value.7 = true,
        }
        assert!(check(value).is_err());
    }
    let mut final_generation = valid;
    final_generation.5 = u64::MAX - 1;
    final_generation.6 = Some(u64::MAX - 1);
    assert!(check(final_generation).is_ok());
}

#[test]
fn scoped_bank_counts_are_separate_exact_window_data() {
    let mut fake = ScopedFake::new();
    assert_eq!(transact(&mut fake, 36, Mode::Rearm, 10, None).unwrap(), 2);
    assert_eq!(fake.inner.calls, 329);
    assert_eq!(fake.local, 361);
    let good = fake.counts();
    assert!(scoped::checked_counts(good).is_ok());
    for field in 0..5 {
        let mut bad = fake.counts();
        match field {
            0 => bad.full_discoveries += 1,
            1 => bad.local_checkpoints += 1,
            2 => bad.before_calls += 1,
            3 => bad.after_calls += 1,
            _ => bad.generation_probes += 1,
        }
        assert!(scoped::checked_counts(bad).is_err());
    }
    assert_eq!(fake.boundary, scoped::Boundary::Closed);
    assert!(scoped::next_boundary(fake.boundary).is_err());
}

#[test]
fn scoped_bank_validates_all_36_before_first_reset_and_commits_after_exit() {
    let mut fake = ScopedFake::new();
    transact(&mut fake, 36, Mode::Rearm, 10, None).unwrap();
    let events = &fake.inner.events;
    assert!(
        events[2..38]
            .iter()
            .all(|e| matches!(e, Event::Identity(_)))
    );
    assert_eq!(events[147], Event::ResetPrefix(0, 0));
    assert_eq!(events[146], Event::Fence);
    assert_eq!(events[326], Event::Initial(35));
    assert_eq!(events[327], Event::Fence);
    assert_eq!(events[328], Event::Commit);
    assert!(fake.inner.published && fake.inner.quarantined == 0);
}

#[test]
fn scoped_bank_stale_generation_duplicate_and_last_rank_refuse_before_stores() {
    for defect in 0..7 {
        let mut fake = ScopedFake::new();
        match defect {
            0 => fake.inner.generations[35] = 0,
            1 => fake.inner.generations[35] = 2,
            2 => fake.inner.generations[35] = u64::MAX,
            3 => fake.inner.identities[35][5] = fake.inner.identities[0][0],
            4 => fake.inner.prefixes_good[35][0] = false,
            5 => fake.inner.prefixes_good[35][1] = false,
            _ => fake.inner.proofs_good[35] = false,
        }
        assert!(transact(&mut fake, 36, Mode::Rearm, 10, None).is_err());
        assert!(fake.inner.no_stores());
        assert!(!fake.inner.published && fake.inner.quarantined == 1);
    }
}

#[test]
fn scoped_bank_every_effect_error_and_unwind_quarantines_the_whole_roster() {
    let mut good = ScopedFake::new();
    transact(&mut good, 36, Mode::Rearm, 10, None).unwrap();
    for fail in 1..=good.inner.calls {
        for after in [false, true] {
            for panic in [false, true] {
                let mut fake = ScopedFake::new();
                fake.inner.fail = fail;
                fake.inner.after = after;
                fake.inner.panic = panic;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    transact(&mut fake, 36, Mode::Rearm, 10, None)
                }));
                if panic {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(fake.inner.calls, fail);
                assert_eq!(fake.inner.events, good.inner.events[..fail]);
                assert!(!fake.inner.published && fake.inner.quarantined == 1);
            }
        }
    }
}

#[test]
fn scoped_bank_every_effect_deadline_including_exit_and_commit_is_fail_stop() {
    for expire in 1..=329 {
        for outer in [false, true] {
            let mut fake = ScopedFake::new();
            fake.inner.expire = expire;
            let until = outer.then_some(fake.inner.clock + Duration::from_millis(5));
            assert!(transact(&mut fake, 36, Mode::Rearm, 10, until).is_err());
            assert_eq!(fake.inner.calls, expire);
            assert!(!fake.inner.published && fake.inner.quarantined == 1);
        }
    }
    for timeout in [0, 10_001, u32::MAX] {
        let mut fake = ScopedFake::new();
        assert!(transact(&mut fake, 36, Mode::Rearm, timeout, None).is_err());
        assert_eq!(fake.inner.calls, 0);
        assert_eq!(fake.inner.quarantined, 1);
    }
}

#[test]
fn scoped_bank_native_first_use_and_invalid_deadline_quarantine_every_owner() {
    for count in [0, 1, 35, 36, 37] {
        for timeout in [0, 10, 10_001] {
            for shared in [false, true] {
                let mut group = native_group();
                group.shared_full_currentness = shared;
                let mut pairs: Vec<_> = (0..count).map(native_pair).collect();
                let mut prefixes: Vec<_> = (0..count).map(native_prefixes).collect();
                let mut entries: Vec<_> = prefixes
                    .iter_mut()
                    .zip(pairs.iter_mut())
                    .map(|(prefixes, pair)| {
                        let [left, right] = prefixes;
                        Entry {
                            prefixes: [left, right],
                            pair,
                        }
                    })
                    .collect();
                // No contexts or retained proof: refusal precedes atomic access.
                assert!(
                    unsafe {
                        group.rearm_guarded_mlp_bank_scoped_currentness_unchecked_v1(
                            &mut entries,
                            timeout,
                        )
                    }
                    .is_err()
                );
                drop(entries);
                assert!(group.poisoned && group.buffers.is_empty());
                assert_eq!(group.next_buffer, 5);
                assert!(pairs.iter().all(|p| {
                    p.phase == Phase::Poisoned
                        && p.owners
                            .iter()
                            .all(|o| o.activation == Activation::Poisoned)
                }));
                assert!(
                    prefixes
                        .iter()
                        .flatten()
                        .all(|p| p.activation == prefix_state::Activation::Submitted)
                );
            }
        }
    }
}
