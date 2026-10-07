//! Fixed-36 scoped rearm; no shader, user callback or lasting currentness policy.
use super::*;

/// Data from one bank-rearm window, not a layer count or a retirement proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerScopedBankRearmObservationV1 {
    pub generation: u64,
    pub entries: u32,
    pub currentness: Gfx950EngineeringPeerScopedCurrentnessCountsV1,
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    pub currentness_durations: crate::Gfx950EngineeringCurrentnessDurationsV1,
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    pub bank_guarded_body_ns: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Boundary {
    Entry,
    BeforeReset,
    Exit,
    Closed,
}

pub(super) fn next_boundary(boundary: Boundary) -> Result<Boundary> {
    match boundary {
        Boundary::Entry => Ok(Boundary::BeforeReset),
        Boundary::BeforeReset => Ok(Boundary::Exit),
        Boundary::Exit => Ok(Boundary::Closed),
        Boundary::Closed => Err("scoped bank boundary repeated after full exit".into()),
    }
}

pub(super) fn admission(
    count: usize,
    mode: Mode,
    phase: Phase,
    arena: ArenaPolicy,
    output: profiles::OutputPolicy,
    generation: u64,
    completed: Option<u64>,
    reusable: bool,
) -> Result<()> {
    if count != MAX_ENTRIES
        || mode != Mode::Rearm
        || phase != Phase::Completed
        || arena != ArenaPolicy::ReuseRetired
        || output != profiles::OutputPolicy::ExactOwnResidual
        || generation == 0
        || generation == u64::MAX
        || completed != Some(generation)
        || reusable
    {
        return Err(
            "scoped bank requires36 genuine completed reusable exact-residual pairs".into(),
        );
    }
    Ok(())
}

pub(super) fn checked_counts(
    counts: crate::device::ScopedCountsV1,
) -> Result<Gfx950EngineeringPeerScopedCurrentnessCountsV1> {
    // Thirty-six proof rechecks (two boundaries), 72 Prefix resets and 72 MLP
    // resets (two each), and the whole-roster pre-reset boundary. All are group
    // checkpoints, not selected-rank checks. This counts only the bank window.
    if counts.full_discoveries != 2
        || counts.local_checkpoints != 361
        || counts.before_calls != 726
        || counts.after_calls != 726
        || counts.generation_probes != 725
    {
        return Err("scoped bank checkpoint census changed".into());
    }
    Ok(counts.into())
}

struct ScopedNative<'group, 'entries, 'owners> {
    native: Native<'group, 'entries, 'owners>,
    until: Option<Instant>,
    window: Option<scoped_currentness::Window>,
    boundary: Boundary,
    counts: Option<Gfx950EngineeringPeerScopedCurrentnessCountsV1>,
    committed: bool,
}

impl Drop for ScopedNative<'_, '_, '_> {
    fn drop(&mut self) {
        if !self.committed {
            // This outer owner also covers deadline construction and the final
            // data-result construction after transact's inner custody returns.
            self.quarantine();
        }
    }
}

impl ScopedNative<'_, '_, '_> {
    fn read_prefix(&mut self, index: usize, rank: usize) -> Result<[u32; 284]> {
        if self.window.is_none() || self.boundary == Boundary::Closed {
            return Err("scoped bank observation outside its window".into());
        }
        // SAFETY: this owner retains the complete typed roster and Group from
        // full entry through full exit and quarantines all on refusal/unwind.
        unsafe {
            prefix_state::observe_within_scoped_bank(
                self.native.group,
                &*self.native.entries[index].prefixes[rank],
            )
        }
    }
}

impl BankBackend for ScopedNative<'_, '_, '_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn enter(&mut self, mode: Mode) -> Result<()> {
        if self.native.group.shared_full_currentness || self.native.entries.len() != MAX_ENTRIES {
            return Err("scoped bank requires Default Group policy and exactly36 entries".into());
        }
        for entry in &*self.native.entries {
            admission(
                self.native.entries.len(),
                mode,
                entry.pair.phase,
                entry.pair.arena_policy,
                entry.pair.binding.output_policy,
                entry.pair.owners[0].generation,
                entry.pair.completed.as_ref().map(|old| old.generation),
                entry.pair.reusable.is_some(),
            )?;
        }
        self.native.enter(mode)
    }

    fn fence(&mut self) -> Result<()> {
        let next = next_boundary(self.boundary)?;
        match self.boundary {
            Boundary::Entry => {
                self.window = Some(scoped_currentness::Window::enter(
                    self.native.group,
                    self.until.ok_or("scoped bank deadline missing")?,
                )?);
            }
            Boundary::BeforeReset => {
                scoped_currentness::Currentness::Scoped(
                    self.window.as_mut().ok_or("scoped bank window missing")?,
                )
                .idle_group(self.native.group)?;
            }
            Boundary::Exit => {
                let counts = self
                    .window
                    .as_mut()
                    .ok_or("scoped bank window missing")?
                    .finish(self.native.group)?;
                self.counts = Some(checked_counts(counts)?);
            }
            Boundary::Closed => unreachable!("next_boundary refused closed window"),
        }
        self.boundary = next;
        Ok(())
    }

    fn identity(&mut self, index: usize, mode: Mode) -> Result<()> {
        self.native.identity(index, mode)
    }

    fn prefix(&mut self, index: usize, rank: usize, mode: Mode) -> Result<()> {
        if mode != Mode::Rearm {
            return Err("scoped bank first-use route refused".into());
        }
        let words = self.read_prefix(index, rank)?;
        prefix_profile::validate_final_state(words)?;
        self.native.prefixes[index][rank] = Some(words);
        Ok(())
    }

    fn pair(&mut self, index: usize, mode: Mode, until: Instant) -> Result<u64> {
        if mode != Mode::Rearm {
            return Err("scoped bank first-use proof refused".into());
        }
        validate_completed_pair_currentness(
            self.native.group,
            &mut *self.native.entries[index].pair,
            &mut self.native.owner_ids,
            &mut self.native.arena_ids,
            until,
            &mut scoped_currentness::Currentness::Scoped(
                self.window.as_mut().ok_or("scoped bank window missing")?,
            ),
        )
    }

    fn reset_prefix(&mut self, index: usize, rank: usize) -> Result<()> {
        let expected = self.native.prefixes[index][rank]
            .as_ref()
            .ok_or("scoped bank Prefix snapshot missing")?;
        // SAFETY: identical whole-bank old-command retirement and validation
        // order as ordinary rearm; only the named scoped cadence differs.
        unsafe {
            prefix_state::rearm_within_scoped_bank(
                self.native.group,
                &mut *self.native.entries[index].prefixes[rank],
                expected,
                &mut scoped_currentness::Currentness::Scoped(
                    self.window.as_mut().ok_or("scoped bank window missing")?,
                ),
            )
        }
    }

    fn reset_pair(&mut self, index: usize, rank: usize, next: u64) -> Result<()> {
        let pair = &mut *self.native.entries[index].pair;
        let old = pair
            .completed
            .as_ref()
            .ok_or("scoped bank reset lost proof")?;
        // SAFETY: every owner/proof and the whole-roster pre-reset checkpoint
        // passed before any store; the same exclusive custody spans all resets.
        unsafe {
            pair.owners[rank].rearm_quiescent_currentness(
                self.native.group,
                &old.states[rank],
                next,
                &mut scoped_currentness::Currentness::Scoped(
                    self.window.as_mut().ok_or("scoped bank window missing")?,
                ),
            )
        }
    }

    fn initial(&mut self, index: usize, next: u64) -> Result<()> {
        for rank in 0..2 {
            prefix_identity(
                self.native.group,
                &*self.native.entries[index].prefixes[rank],
                rank,
                Mode::Initial,
            )?;
            if self.read_prefix(index, rank)? != prefix_profile::INITIAL_STATE {
                return Err("scoped bank Prefix initial readback mismatch".into());
            }
        }
        if profiles::validate_owners(self.native.group, &self.native.entries[index].pair.owners)?
            != next
        {
            return Err("scoped bank MLP initial readback mismatch".into());
        }
        Ok(())
    }

    fn commit(&mut self, mode: Mode, next: u64) -> Result<()> {
        if self.boundary != Boundary::Closed || self.counts.is_none() {
            return Err("scoped bank commit requires successful full exit".into());
        }
        self.native.commit(mode, next)
    }

    fn quarantine(&mut self) {
        self.native.quarantine();
        for context in &mut self.native.group.contexts {
            crate::device::ScopedCurrentnessV1::poison_devices(&mut [context
                .backend
                .engineering_peer_device()]);
        }
    }
}

/// # Safety
/// Same whole-bank permanent retirement obligations as ordinary mixed rearm;
/// the caller deliberately accepts scoped temporal sampling for this call.
pub(in crate::engineering_gfx950) unsafe fn rearm_bank_scoped(
    group: &mut Gfx950EngineeringPeerGroupV1,
    entries: &mut [Entry<'_>],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringPeerScopedBankRearmObservationV1> {
    let count = entries.len();
    let mut backend = ScopedNative {
        native: Native {
            group,
            entries,
            prefixes: Vec::new(),
            all_ids: BTreeSet::new(),
            owner_ids: BTreeSet::new(),
            arena_ids: BTreeSet::new(),
        },
        until: None,
        window: None,
        boundary: Boundary::Entry,
        counts: None,
        committed: false,
    };
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    let diagnostic_started = backend.now();
    let until = deadline(backend.now(), timeout_ms)?;
    backend.until = Some(until);
    let generation = transact(&mut backend, count, Mode::Rearm, timeout_ms, Some(until))?;
    let result = Gfx950EngineeringPeerScopedBankRearmObservationV1 {
        generation,
        entries: MAX_ENTRIES as u32,
        currentness: backend
            .counts
            .ok_or("scoped bank result lacks full-exit counts")?,
        #[cfg(feature = "engineering-currentness-duration-diagnostics")]
        currentness_durations: backend
            .window
            .as_ref()
            .ok_or("scoped bank duration window absent")?
            .durations()?,
        #[cfg(feature = "engineering-currentness-duration-diagnostics")]
        bank_guarded_body_ns: 0,
    };
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    let result = {
        let mut result = result;
        result.bank_guarded_body_ns = result
            .currentness_durations
            .bank_interval_ns(diagnostic_started, backend.now())
            .map_err(explain)?;
        result
    };
    deadline_check(backend.now(), until)?;
    backend.committed = true;
    Ok(result)
}

#[cfg(test)]
mod custody_tests {
    use super::super::tests::{native_group, native_pair, native_prefixes};
    use super::*;

    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    #[test]
    fn scoped_bank_duration_refusal_and_unwind_preserve_outer_custody() {
        for fault in 0..4 {
            let mut group = native_group();
            let mut pair = native_pair(0);
            let mut prefixes = native_prefixes(0);
            let [left, right] = &mut prefixes;
            let mut entries = [Entry {
                prefixes: [left, right],
                pair: &mut pair,
            }];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut backend = ScopedNative {
                    native: Native {
                        group: &mut group,
                        entries: &mut entries,
                        prefixes: Vec::new(),
                        all_ids: BTreeSet::new(),
                        owner_ids: BTreeSet::new(),
                        arena_ids: BTreeSet::new(),
                    },
                    until: None,
                    window: None,
                    boundary: Boundary::Closed,
                    counts: None,
                    committed: false,
                };
                backend.native.entries[0].pair.phase = Phase::Ready;
                let start = Instant::now();
                let mut totals = crate::Gfx950EngineeringCurrentnessDurationsV1::default();
                totals.before.elapsed_ns = 1;
                let refused = match fault {
                    0 => totals.bank_interval_ns(start, start),
                    1 => {
                        totals.before.elapsed_ns = u64::MAX;
                        totals.after.elapsed_ns = 1;
                        totals.bank_interval_ns(start, start)
                    }
                    2 => totals.bank_interval_ns(
                        start,
                        start
                            .checked_sub(std::time::Duration::from_nanos(1))
                            .unwrap(),
                    ),
                    _ => panic!("diagnostic bank commit-tail unwind"),
                };
                assert!(refused.is_err());
            }));
            assert_eq!(result.is_err(), fault == 3);
            drop(entries);
            assert!(group.poisoned);
            assert_eq!(pair.phase, Phase::Poisoned);
            assert!(
                pair.owners
                    .iter()
                    .all(|v| v.activation == Activation::Poisoned)
            );
            assert!(
                prefixes
                    .iter()
                    .all(|v| v.activation == prefix_state::Activation::Submitted)
            );
        }
    }

    #[test]
    fn scoped_bank_outer_owner_quarantines_partial_commit_and_unwind() {
        for unwind in [false, true] {
            let mut group = native_group();
            let mut pair = native_pair(0);
            let mut prefixes = native_prefixes(0);
            let [left, right] = &mut prefixes;
            let mut entries = [Entry {
                prefixes: [left, right],
                pair: &mut pair,
            }];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut backend = ScopedNative {
                    native: Native {
                        group: &mut group,
                        entries: &mut entries,
                        prefixes: Vec::new(),
                        all_ids: BTreeSet::new(),
                        owner_ids: BTreeSet::new(),
                        arena_ids: BTreeSet::new(),
                    },
                    until: None,
                    window: None,
                    boundary: Boundary::Closed,
                    counts: None,
                    committed: false,
                };
                // Model the commit tail, where internal metadata is already
                // Ready but no successful data result may escape outer custody.
                backend.native.entries[0].pair.phase = Phase::Ready;
                if unwind {
                    panic!("scoped bank commit-tail unwind");
                }
            }));
            assert_eq!(result.is_err(), unwind);
            drop(entries);
            assert!(group.poisoned);
            assert_eq!(pair.phase, Phase::Poisoned);
            assert!(
                pair.owners
                    .iter()
                    .all(|owner| owner.activation == Activation::Poisoned)
            );
            assert!(
                prefixes
                    .iter()
                    .all(|prefix| prefix.activation == prefix_state::Activation::Submitted)
            );
        }
    }
}
