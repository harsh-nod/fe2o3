use super::*;

#[path = "engineering_gfx950_peer_scoped_bank_rearm_v1_tests.rs"]
mod scoped_bank_tests;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Enter,
    Fence,
    Identity(usize),
    Prefix(usize, usize),
    Pair(usize),
    ResetPrefix(usize, usize),
    ResetPair(usize, usize),
    Initial(usize),
    Commit,
}

struct Fake {
    clock: Instant,
    mode: Mode,
    calls: usize,
    fail: usize,
    after: bool,
    panic: bool,
    expire: usize,
    generations: Vec<u64>,
    identities: Vec<[u64; 6]>,
    seen: BTreeSet<(u64, u64)>,
    prefixes_good: Vec<[bool; 2]>,
    proofs_good: Vec<bool>,
    identities_checked: Vec<bool>,
    prefixes_checked: Vec<[bool; 2]>,
    pairs_checked: Vec<bool>,
    prefix_resets: Vec<[bool; 2]>,
    pair_resets: Vec<[bool; 2]>,
    initial_checked: Vec<bool>,
    fences: usize,
    entered: bool,
    published: bool,
    quarantined: usize,
    events: Vec<Event>,
}

impl Fake {
    fn new(count: usize, mode: Mode) -> Self {
        Self {
            clock: Instant::now(),
            mode,
            calls: 0,
            fail: 0,
            after: false,
            panic: false,
            expire: 0,
            generations: vec![1; count],
            identities: (0..count)
                .map(|i| std::array::from_fn(|j| (i * 6 + j + 1) as u64))
                .collect(),
            seen: BTreeSet::new(),
            prefixes_good: vec![[true; 2]; count],
            proofs_good: vec![true; count],
            identities_checked: vec![false; count],
            prefixes_checked: vec![[false; 2]; count],
            pairs_checked: vec![false; count],
            prefix_resets: vec![[false; 2]; count],
            pair_resets: vec![[false; 2]; count],
            initial_checked: vec![false; count],
            fences: 0,
            entered: false,
            published: false,
            quarantined: 0,
            events: Vec::new(),
        }
    }

    fn fault(&self) -> Result<()> {
        if self.panic {
            panic!("mixed bank injected unwind");
        }
        Err("mixed bank injected failure".into())
    }

    fn step(&mut self, event: Event, effect: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        assert_eq!(self.quarantined, 0);
        self.calls += 1;
        self.events.push(event);
        if self.calls == self.fail && !self.after {
            return self.fault();
        }
        effect(self)?;
        if self.calls == self.expire {
            self.clock += Duration::from_millis(10);
        }
        if self.calls == self.fail && self.after {
            return self.fault();
        }
        Ok(())
    }

    fn no_stores(&self) -> bool {
        self.prefix_resets
            .iter()
            .chain(self.pair_resets.iter())
            .all(|r| *r == [false; 2])
    }

    fn all_validated(&self) -> bool {
        self.identities_checked.iter().all(|v| *v)
            && self.prefixes_checked.iter().all(|v| *v == [true; 2])
            && self.pairs_checked.iter().all(|v| *v)
    }

    fn require_reset_ready(&self) {
        assert_eq!(self.mode, Mode::Rearm);
        assert!(self.all_validated());
        assert_eq!(self.fences, 2);
        assert!(!self.published);
    }
}

impl BankBackend for Fake {
    fn now(&mut self) -> Instant {
        self.clock
    }

    fn enter(&mut self, mode: Mode) -> Result<()> {
        self.step(Event::Enter, |s| {
            assert_eq!(mode, s.mode);
            assert!(!s.entered);
            s.entered = true;
            Ok(())
        })
    }

    fn fence(&mut self) -> Result<()> {
        self.step(Event::Fence, |s| {
            assert!(s.entered);
            match s.fences {
                0 => assert!(s.no_stores()),
                1 => {
                    assert!(s.all_validated());
                    assert!(s.no_stores());
                }
                2 => assert!(s.initial_checked.iter().all(|v| *v)),
                _ => panic!("unexpected mixed bank fence"),
            }
            s.fences += 1;
            Ok(())
        })
    }

    fn identity(&mut self, index: usize, mode: Mode) -> Result<()> {
        self.step(Event::Identity(index), |s| {
            assert_eq!(mode, s.mode);
            assert_eq!(s.fences, 1);
            assert!(s.prefixes_checked.iter().all(|v| *v == [false; 2]));
            let count = if mode == Mode::Initial { 4 } else { 6 };
            for slot in 0..count {
                remember_identity(&mut s.seen, 7, s.identities[index][slot])?;
            }
            s.identities_checked[index] = true;
            Ok(())
        })
    }

    fn prefix(&mut self, index: usize, rank: usize, mode: Mode) -> Result<()> {
        self.step(Event::Prefix(index, rank), |s| {
            assert_eq!(mode, s.mode);
            assert!(s.identities_checked.iter().all(|v| *v));
            assert!(s.no_stores());
            if !s.prefixes_good[index][rank] {
                return Err("invalid typed prefix state".into());
            }
            s.prefixes_checked[index][rank] = true;
            Ok(())
        })
    }

    fn pair(&mut self, index: usize, mode: Mode, until: Instant) -> Result<u64> {
        deadline_check(self.clock, until)?;
        self.step(Event::Pair(index), |s| {
            assert_eq!(mode, s.mode);
            assert_eq!(s.prefixes_checked[index], [true; 2]);
            assert!(s.no_stores());
            if !s.proofs_good[index] {
                return Err("invalid sealed pair proof or contents".into());
            }
            s.pairs_checked[index] = true;
            Ok(())
        })?;
        Ok(self.generations[index])
    }

    fn reset_prefix(&mut self, index: usize, rank: usize) -> Result<()> {
        self.step(Event::ResetPrefix(index, rank), |s| {
            s.require_reset_ready();
            assert!(!s.prefix_resets[index][rank]);
            s.prefix_resets[index][rank] = true;
            Ok(())
        })
    }

    fn reset_pair(&mut self, index: usize, rank: usize, next: u64) -> Result<()> {
        self.step(Event::ResetPair(index, rank), |s| {
            s.require_reset_ready();
            assert!(s.prefix_resets[index][rank]);
            assert!(!s.pair_resets[index][rank]);
            assert_eq!(next, s.generations[index] + 1);
            s.pair_resets[index][rank] = true;
            Ok(())
        })
    }

    fn initial(&mut self, index: usize, next: u64) -> Result<()> {
        self.step(Event::Initial(index), |s| {
            assert_eq!(s.fences, 2);
            assert!(
                s.prefix_resets
                    .iter()
                    .chain(s.pair_resets.iter())
                    .all(|v| *v == [true; 2])
            );
            assert_eq!(next, s.generations[index] + 1);
            s.initial_checked[index] = true;
            Ok(())
        })
    }

    fn commit(&mut self, mode: Mode, next: u64) -> Result<()> {
        self.step(Event::Commit, |s| {
            assert_eq!(mode, s.mode);
            assert!(s.all_validated());
            match mode {
                Mode::Initial => {
                    assert_eq!(next, 1);
                    assert_eq!(s.fences, 2);
                    assert!(s.no_stores());
                }
                Mode::Rearm => {
                    assert_eq!(s.fences, 3);
                    assert!(s.initial_checked.iter().all(|v| *v));
                }
            }
            s.published = true;
            Ok(())
        })
    }

    fn quarantine(&mut self) {
        self.quarantined += 1;
        self.published = false;
    }
}

#[test]
fn mixed_bank_validates_every_owner_before_stores_and_publishes_last() {
    for count in [1, 2, 36] {
        let mut fake = Fake::new(count, Mode::Rearm);
        assert_eq!(
            transact(&mut fake, count, Mode::Rearm, 10, None).unwrap(),
            2
        );
        assert_eq!(fake.calls, 9 * count + 5);
        assert_eq!(fake.events.last(), Some(&Event::Commit));
        assert_eq!(fake.fences, 3);
        assert_eq!(fake.seen.len(), 6 * count);
        assert!(fake.published && fake.quarantined == 0);
        let first_store = fake
            .events
            .iter()
            .position(|e| matches!(e, Event::ResetPrefix(_, _)))
            .unwrap();
        assert_eq!(first_store, 4 * count + 3);
        assert_eq!(fake.events[first_store - 1], Event::Fence);
    }
}

#[test]
fn mixed_bank_initial_validation_has_no_stores_or_retired_arenas() {
    for count in [1, 2, 36] {
        let mut fake = Fake::new(count, Mode::Initial);
        assert_eq!(
            transact(&mut fake, count, Mode::Initial, 10, None).unwrap(),
            1
        );
        assert_eq!(fake.calls, 4 * count + 4);
        assert_eq!(fake.seen.len(), 4 * count);
        assert!(fake.no_stores());
        assert!(fake.published && fake.quarantined == 0);
        assert!(!fake.events.iter().any(|e| matches!(e, Event::Initial(_))));
    }
}

#[test]
fn mixed_bank_bounds_timeouts_and_outer_deadline_refuse_before_effects() {
    for mode in [Mode::Initial, Mode::Rearm] {
        for count in [0, 37, usize::MAX] {
            let mut fake = Fake::new(36, mode);
            assert!(transact(&mut fake, count, mode, 10, None).is_err());
            assert_eq!(fake.calls, 0);
            assert_eq!(fake.quarantined, 1);
        }
        for timeout in [0, 10_001, u32::MAX] {
            let mut fake = Fake::new(1, mode);
            assert!(transact(&mut fake, 1, mode, timeout, None).is_err());
            assert_eq!(fake.calls, 0);
            assert_eq!(fake.quarantined, 1);
        }
        let mut fake = Fake::new(1, mode);
        let cutoff = fake.clock;
        assert!(transact(&mut fake, 1, mode, 10, Some(cutoff)).is_err());
        assert_eq!(fake.calls, 0);
        assert_eq!(fake.quarantined, 1);
    }
}

#[test]
fn mixed_bank_last_invalid_prefix_or_pair_performs_zero_stores() {
    for mode in [Mode::Initial, Mode::Rearm] {
        for defect in 0..3 {
            let mut fake = Fake::new(36, mode);
            if defect < 2 {
                fake.prefixes_good[35][defect] = false;
            } else {
                fake.proofs_good[35] = false;
            }
            assert!(transact(&mut fake, 36, mode, 10, None).is_err());
            assert!(fake.no_stores());
            assert!(!fake.published && fake.quarantined == 1);
            assert!(!fake.events.iter().any(|e| matches!(e, Event::Commit)));
        }
    }
}

#[test]
fn mixed_bank_cross_kind_identity_collisions_refuse_before_atomic_reads() {
    for mode in [Mode::Initial, Mode::Rearm] {
        let slots = if mode == Mode::Initial { 4 } else { 6 };
        for left in 0..slots {
            for right in 0..slots {
                let mut fake = Fake::new(36, mode);
                fake.identities[35][right] = fake.identities[0][left];
                assert!(transact(&mut fake, 36, mode, 10, None).is_err());
                assert!(fake.no_stores());
                assert!(fake.prefixes_checked.iter().all(|v| *v == [false; 2]));
                assert_eq!(fake.quarantined, 1);
            }
        }
        let mut fake = Fake::new(1, mode);
        fake.identities[0][0] = 0;
        assert!(transact(&mut fake, 1, mode, 10, None).is_err());
        assert!(fake.no_stores());
    }
    let mut ids = BTreeSet::new();
    assert!(remember_identity(&mut ids, 0, 1).is_err());
    assert!(remember_identity(&mut ids, 7, 0).is_err());
    assert!(ids.is_empty());
}

#[test]
fn mixed_bank_generations_require_one_common_nonzero_successor() {
    for mode in [Mode::Initial, Mode::Rearm] {
        for bad in [0, 2, u64::MAX] {
            let mut fake = Fake::new(36, mode);
            fake.generations[35] = bad;
            assert!(transact(&mut fake, 36, mode, 10, None).is_err());
            assert!(fake.no_stores());
            assert_eq!(fake.quarantined, 1);
        }
    }
    let mut fake = Fake::new(2, Mode::Rearm);
    fake.generations.fill(u64::MAX - 1);
    assert_eq!(
        transact(&mut fake, 2, Mode::Rearm, 10, None).unwrap(),
        u64::MAX
    );
}

#[test]
fn mixed_bank_every_error_boundary_quarantines_without_later_effects() {
    for mode in [Mode::Initial, Mode::Rearm] {
        for count in [2, 36] {
            let mut good = Fake::new(count, mode);
            transact(&mut good, count, mode, 10, None).unwrap();
            for fail in 1..=good.calls {
                for after in [false, true] {
                    let mut fake = Fake::new(count, mode);
                    fake.fail = fail;
                    fake.after = after;
                    assert!(transact(&mut fake, count, mode, 10, None).is_err());
                    assert_eq!(fake.calls, fail);
                    assert_eq!(fake.events, good.events[..fail]);
                    assert!(!fake.published && fake.quarantined == 1);
                }
            }
        }
    }
}

#[test]
fn mixed_bank_every_unwind_boundary_quarantines_without_publishing() {
    for mode in [Mode::Initial, Mode::Rearm] {
        let mut good = Fake::new(2, mode);
        transact(&mut good, 2, mode, 10, None).unwrap();
        for fail in 1..=good.calls {
            for after in [false, true] {
                let mut fake = Fake::new(2, mode);
                fake.fail = fail;
                fake.after = after;
                fake.panic = true;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    transact(&mut fake, 2, mode, 10, None)
                }));
                assert!(result.is_err());
                assert_eq!(fake.calls, fail);
                assert!(!fake.published && fake.quarantined == 1);
            }
        }
    }
}

#[test]
fn mixed_bank_every_deadline_boundary_blocks_the_next_operation() {
    for mode in [Mode::Initial, Mode::Rearm] {
        let mut good = Fake::new(2, mode);
        transact(&mut good, 2, mode, 10, None).unwrap();
        for expire in 1..=good.calls {
            for outer in [false, true] {
                let mut fake = Fake::new(2, mode);
                fake.expire = expire;
                let cutoff = outer.then_some(fake.clock + Duration::from_millis(5));
                assert!(transact(&mut fake, 2, mode, 10, cutoff).is_err());
                assert_eq!(fake.calls, expire);
                assert_eq!(fake.events, good.events[..expire]);
                assert!(!fake.published && fake.quarantined == 1);
            }
        }
        let mut fake = Fake::new(2, mode);
        fake.expire = 1;
        let later = fake.clock + Duration::from_secs(1);
        assert!(transact(&mut fake, 2, mode, 10, Some(later)).is_err());
        assert_eq!(fake.calls, 1);
    }
}

pub(super) fn native_group() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: BTreeMap::new(),
        next_buffer: 5,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    }
}

fn token(rank: usize, id: u64, bytes: u64) -> Gfx950EngineeringPeerBufferV1 {
    Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id,
        owner: rank,
        bytes,
    }
}

pub(super) fn native_pair(index: usize) -> RetainedPair {
    RetainedPair {
        owners: std::array::from_fn(|rank| CombinedMlpStateV1 {
            buffer: token(rank, (4 * index + rank + 3) as u64, 2208),
            activation: Activation::Ready,
            generation: 1,
        }),
        binding: Binding {
            group: 7,
            output_policy: profiles::OutputPolicy::Strict,
            kernels: [[(7, 0, 1, [1; 32]); 4]; 2],
            roots: std::array::from_fn(|rank| {
                std::array::from_fn(|j| token(rank, 1000 + j as u64, 8192))
            }),
            partials: [token(0, 2000, 16384), token(1, 2001, 16384)],
            residuals: [token(0, 2002, 8192), token(1, 2003, 8192)],
            outputs: [token(0, 2004, 8192), token(1, 2005, 8192)],
            images: [[1; 32]; 2],
        },
        phase: Phase::Ready,
        completed: None,
        arena_policy: ArenaPolicy::Fresh,
        reusable: None,
    }
}

#[test]
fn mixed_bank_reuse_custody_is_unavailable_until_all_36_pairs_pass() {
    // Production commit is the only transfer of a completed pair into its
    // pending-reuse slot. No signal reset happens in this transaction.
    for failed_pair in 0..36 {
        let mut fake = Fake::new(36, Mode::Rearm);
        fake.proofs_good[failed_pair] = false;
        assert!(transact(&mut fake, 36, Mode::Rearm, 10, None).is_err());
        assert!(fake.no_stores());
        assert!(!fake.events.contains(&Event::Commit));
        assert_eq!(fake.quarantined, 1);
    }
    let mut fake = Fake::new(36, Mode::Rearm);
    assert_eq!(transact(&mut fake, 36, Mode::Rearm, 10, None).unwrap(), 2);
    assert!(fake.all_validated());
    assert_eq!(fake.events.last(), Some(&Event::Commit));
    assert_eq!(fake.quarantined, 0);
}

pub(super) fn native_prefixes(index: usize) -> [Prefix; 2] {
    std::array::from_fn(|rank| Prefix {
        buffer: token(rank, (4 * index + rank + 1) as u64, 1136),
        activation: prefix_state::Activation::Ready,
    })
}

#[test]
fn mixed_bank_native_refuses_context_free_custody_and_quarantines_all_entries() {
    for mode in [Mode::Initial, Mode::Rearm] {
        for count in [0, 1, 2, 36, 37] {
            for timeout in [0, 10, 10_001] {
                let mut group = native_group();
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
                let result = match mode {
                    Mode::Initial => initial_bank(&mut group, &mut entries, timeout),
                    // No GPU contexts exist: refusal precedes atomic access or reset.
                    Mode::Rearm => unsafe { rearm_bank(&mut group, &mut entries, timeout) },
                };
                assert!(result.is_err());
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

#[test]
fn mixed_bank_native_prefix_metadata_rejects_wrong_phase_rank_group_and_extent() {
    let group = native_group();
    for mode in [Mode::Initial, Mode::Rearm] {
        for rank in 0..2 {
            for bad in 0..5 {
                let mut prefixes = native_prefixes(0);
                let prefix = &mut prefixes[rank];
                prefix.activation = mode.prefix_phase();
                match bad {
                    0 => prefix.activation = prefix_state::Activation::Submitted,
                    1 => prefix.buffer.owner = 1 - rank,
                    2 => prefix.buffer.group += 1,
                    3 => prefix.buffer.bytes += 4,
                    _ => prefix.buffer.id = 0,
                }
                assert!(
                    prefix_identity(&group, prefix, rank, mode)
                        .unwrap_err()
                        .contains("prefix phase, group or rank")
                );
            }
        }
    }
}

#[test]
fn mixed_bank_native_unwind_keeps_group_and_typed_owners_quarantined() {
    let mut group = native_group();
    let mut pair = native_pair(0);
    let mut prefixes = native_prefixes(0);
    let [left, right] = &mut prefixes;
    let mut entries = [Entry {
        prefixes: [left, right],
        pair: &mut pair,
    }];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut backend = Native {
            group: &mut group,
            entries: &mut entries,
            prefixes: Vec::new(),
            all_ids: BTreeSet::new(),
            owner_ids: BTreeSet::new(),
            arena_ids: BTreeSet::new(),
        };
        let _custody = Custody {
            backend: &mut backend,
            committed: false,
        };
        panic!("native mixed bank custody unwind");
    }));
    assert!(result.is_err());
    drop(entries);
    assert!(group.poisoned);
    assert_eq!(pair.phase, Phase::Poisoned);
    assert!(
        pair.owners
            .iter()
            .all(|o| o.activation == Activation::Poisoned)
    );
    assert!(
        prefixes
            .iter()
            .all(|p| p.activation == prefix_state::Activation::Submitted)
    );
}
