//! One exclusive, fail-stop transaction for Prefix284 and guarded MLP owners.
use super::super::super::super::wave_qkv_attention_output_tiles_state_v6 as prefix_state;
use super::*;
use crate::engineering_gfx950::wave_qkv_attention_output_tiles_v6 as prefix_profile;

const MAX_ENTRIES: usize = 36;
type Prefix = Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6;

/// Borrowed typed custody, not a snapshot or a reusable completion proof.
/// Rank order is zero, then one; each pair belongs to the caller's same layer.
pub struct Entry<'a> {
    pub prefixes: [&'a mut Prefix; 2],
    pub pair: &'a mut RetainedPair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Initial,
    Rearm,
}

impl Mode {
    fn pair_phase(self) -> Phase {
        match self {
            Self::Initial => Phase::Ready,
            Self::Rearm => Phase::Completed,
        }
    }

    fn prefix_phase(self) -> prefix_state::Activation {
        match self {
            Self::Initial => prefix_state::Activation::Ready,
            Self::Rearm => prefix_state::Activation::Completed,
        }
    }
}

trait BankBackend {
    fn now(&mut self) -> Instant;
    fn enter(&mut self, mode: Mode) -> Result<()>;
    fn fence(&mut self) -> Result<()>;
    fn identity(&mut self, index: usize, mode: Mode) -> Result<()>;
    fn prefix(&mut self, index: usize, rank: usize, mode: Mode) -> Result<()>;
    fn pair(&mut self, index: usize, mode: Mode, until: Instant) -> Result<u64>;
    fn reset_prefix(&mut self, index: usize, rank: usize) -> Result<()>;
    fn reset_pair(&mut self, index: usize, rank: usize, next: u64) -> Result<()>;
    fn initial(&mut self, index: usize, next: u64) -> Result<()>;
    fn commit(&mut self, mode: Mode, next: u64) -> Result<()>;
    fn quarantine(&mut self);
}

struct Custody<'a, B: BankBackend> {
    backend: &'a mut B,
    committed: bool,
}

impl<B: BankBackend> Drop for Custody<'_, B> {
    fn drop(&mut self) {
        if !self.committed {
            self.backend.quarantine();
        }
    }
}

fn step<B: BankBackend, T>(
    backend: &mut B,
    until: Instant,
    action: impl FnOnce(&mut B) -> Result<T>,
) -> Result<T> {
    deadline_check(backend.now(), until)?;
    let result = action(backend)?;
    deadline_check(backend.now(), until)?;
    Ok(result)
}

fn transact(
    backend: &mut impl BankBackend,
    count: usize,
    mode: Mode,
    timeout_ms: u32,
    outer_deadline: Option<Instant>,
) -> Result<u64> {
    let mut custody = Custody {
        backend,
        committed: false,
    };
    let until = deadline(custody.backend.now(), timeout_ms)?;
    let until = outer_deadline.map_or(until, |outer| outer.min(until));
    deadline_check(custody.backend.now(), until)?;
    if !(1..=MAX_ENTRIES).contains(&count) {
        return Err("mixed guarded bank requires 1..36 entries".into());
    }
    step(custody.backend, until, |b| b.enter(mode))?;
    step(custody.backend, until, |b| b.fence())?;
    // Authenticate the entire typed roster before reading its atomic contents.
    for index in 0..count {
        step(custody.backend, until, |b| b.identity(index, mode))?;
    }
    let mut next = None;
    for index in 0..count {
        for rank in 0..2 {
            step(custody.backend, until, |b| b.prefix(index, rank, mode))?;
        }
        let old = step(custody.backend, until, |b| b.pair(index, mode, until))?;
        let candidate = match mode {
            Mode::Initial if old == 1 => 1,
            Mode::Initial => return Err("mixed guarded initial generation is not one".into()),
            Mode::Rearm => old
                .checked_add(1)
                .filter(|_| old != 0)
                .ok_or("mixed guarded generation exhausted or zero")?,
        };
        if next.is_some_and(|value| value != candidate) {
            return Err("mixed guarded bank generations differ".into());
        }
        next = Some(candidate);
    }
    let next = next.ok_or("mixed guarded bank empty")?;
    // This trailing validation fence must succeed before the first reset.
    step(custody.backend, until, |b| b.fence())?;
    if mode == Mode::Rearm {
        for index in 0..count {
            for rank in 0..2 {
                step(custody.backend, until, |b| b.reset_prefix(index, rank))?;
                step(custody.backend, until, |b| b.reset_pair(index, rank, next))?;
            }
        }
        for index in 0..count {
            step(custody.backend, until, |b| b.initial(index, next))?;
        }
        step(custody.backend, until, |b| b.fence())?;
    }
    step(custody.backend, until, |b| b.commit(mode, next))?;
    custody.committed = true;
    Ok(next)
}

fn remember_identity(ids: &mut BTreeSet<(u64, u64)>, group: u64, id: u64) -> Result<()> {
    if group == 0 || id == 0 || !ids.insert((group, id)) {
        return Err("mixed guarded duplicate or empty storage identity".into());
    }
    Ok(())
}

fn prefix_identity(
    group: &Gfx950EngineeringPeerGroupV1,
    prefix: &Prefix,
    rank: usize,
    mode: Mode,
) -> Result<()> {
    if prefix.activation != mode.prefix_phase()
        || prefix.owner_rank() != rank
        || prefix.buffer.group != group.incarnation
        || prefix.buffer.id == 0
        || prefix.buffer.bytes != prefix_state::STATE_BYTES as u64
    {
        return Err("mixed guarded prefix phase, group or rank mismatch".into());
    }
    prefix_state::validate_idle_bank_state(group, prefix)
}

struct Native<'group, 'entries, 'owners> {
    group: &'group mut Gfx950EngineeringPeerGroupV1,
    entries: &'entries mut [Entry<'owners>],
    prefixes: Vec<[Option<[u32; 284]>; 2]>,
    all_ids: BTreeSet<(u64, u64)>,
    owner_ids: BTreeSet<(u64, u64)>,
    arena_ids: BTreeSet<(u64, u64)>,
}

impl Native<'_, '_, '_> {
    fn read_prefix(&mut self, index: usize, rank: usize) -> Result<[u32; 284]> {
        // SAFETY: transact retains exclusive Group/owner custody from its entry
        // fence through its trailing fence. Errors quarantine before returning.
        unsafe {
            prefix_state::observe_within_idle_bank_fence(
                self.group,
                &*self.entries[index].prefixes[rank],
            )
        }
    }
}

impl BankBackend for Native<'_, '_, '_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn enter(&mut self, mode: Mode) -> Result<()> {
        self.group.require_active()?;
        profiles::validate_policy(self.group)?;
        if self.entries.iter().any(|entry| {
            entry.pair.phase != mode.pair_phase()
                || entry.pair.binding.group != self.group.incarnation
                || match mode {
                    Mode::Initial => entry.pair.completed.is_some(),
                    Mode::Rearm => entry.pair.completed.is_none(),
                }
        }) {
            return Err("mixed guarded bank pair custody mismatch".into());
        }
        self.prefixes
            .try_reserve_exact(self.entries.len())
            .map_err(|_| "mixed guarded prefix snapshot allocation")?;
        self.prefixes.resize(self.entries.len(), [None; 2]);
        for entry in &mut *self.entries {
            entry.pair.phase = Phase::Busy;
        }
        Ok(())
    }

    fn fence(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn identity(&mut self, index: usize, mode: Mode) -> Result<()> {
        let entry = &self.entries[index];
        let expected_generation = match mode {
            Mode::Initial => 1,
            Mode::Rearm => {
                entry
                    .pair
                    .completed
                    .as_ref()
                    .ok_or("mixed guarded completion disappeared")?
                    .generation
            }
        };
        for rank in 0..2 {
            let prefix = &*entry.prefixes[rank];
            prefix_identity(self.group, prefix, rank, mode)?;
            remember_identity(&mut self.all_ids, prefix.buffer.group, prefix.buffer.id)?;
            let owner = &entry.pair.owners[rank];
            if owner.buffer.group != self.group.incarnation
                || owner.buffer.id == 0
                || owner.owner_rank() != rank
                || owner.generation != expected_generation
                || expected_generation == 0
            {
                return Err("mixed guarded combined identity or generation mismatch".into());
            }
            require_activation(
                owner.activation,
                match mode {
                    Mode::Initial => Activation::Ready,
                    Mode::Rearm => Activation::Completed,
                },
            )?;
            owner.local_id(self.group)?;
            remember_identity(&mut self.all_ids, owner.buffer.group, owner.buffer.id)?;
        }
        if mode == Mode::Rearm {
            let old = entry
                .pair
                .completed
                .as_ref()
                .ok_or("mixed guarded proof missing")?;
            for (group, id) in old.proof.identities() {
                if group != self.group.incarnation {
                    return Err("mixed guarded arena belongs to another group".into());
                }
                remember_identity(&mut self.all_ids, group, id)?;
            }
        }
        Ok(())
    }

    fn prefix(&mut self, index: usize, rank: usize, mode: Mode) -> Result<()> {
        let words = self.read_prefix(index, rank)?;
        match mode {
            Mode::Initial if words != prefix_profile::INITIAL_STATE => {
                return Err("mixed guarded prefix is not initial".into());
            }
            Mode::Initial => {}
            Mode::Rearm => prefix_profile::validate_final_state(words)?,
        }
        self.prefixes[index][rank] = Some(words);
        Ok(())
    }

    fn pair(&mut self, index: usize, mode: Mode, until: Instant) -> Result<u64> {
        match mode {
            Mode::Initial => {
                profiles::validate_owners(self.group, &self.entries[index].pair.owners)
            }
            Mode::Rearm => validate_completed_pair(
                self.group,
                &mut *self.entries[index].pair,
                &mut self.owner_ids,
                &mut self.arena_ids,
                until,
            ),
        }
    }

    fn reset_prefix(&mut self, index: usize, rank: usize) -> Result<()> {
        let expected = self.prefixes[index][rank]
            .as_ref()
            .ok_or("mixed guarded prefix snapshot missing")?;
        // SAFETY: the unsafe entry requires permanent retirement of old Prefix
        // commands. All selected states and sealed paired batches were checked
        // before this phase; the same exclusive Group borrow spans all resets.
        unsafe {
            self.group.rearm_wave_qkv_attention_output_tiles_state_v6(
                &mut *self.entries[index].prefixes[rank],
                expected,
            )
        }
    }

    fn reset_pair(&mut self, index: usize, rank: usize, next: u64) -> Result<()> {
        let pair = &mut *self.entries[index].pair;
        let old = pair
            .completed
            .as_ref()
            .ok_or("mixed guarded reset lost proof")?;
        // SAFETY: whole-bank proof/signal/currentness validation and its trailing
        // fence preceded every store. No publication or arena reset can interleave.
        unsafe { pair.owners[rank].rearm_quiescent(self.group, &old.states[rank], next) }
    }

    fn initial(&mut self, index: usize, next: u64) -> Result<()> {
        for rank in 0..2 {
            prefix_identity(
                self.group,
                &*self.entries[index].prefixes[rank],
                rank,
                Mode::Initial,
            )?;
            if self.read_prefix(index, rank)? != prefix_profile::INITIAL_STATE {
                return Err("mixed guarded prefix initial readback mismatch".into());
            }
        }
        if profiles::validate_owners(self.group, &self.entries[index].pair.owners)? != next {
            return Err("mixed guarded combined initial readback mismatch".into());
        }
        Ok(())
    }

    fn commit(&mut self, mode: Mode, next: u64) -> Result<()> {
        if self.entries.iter().any(|entry| {
            entry.pair.phase != Phase::Busy
                || entry
                    .pair
                    .owners
                    .iter()
                    .any(|owner| owner.activation != Activation::Ready || owner.generation != next)
                || entry
                    .prefixes
                    .iter()
                    .any(|prefix| prefix.activation != prefix_state::Activation::Ready)
        }) {
            return Err("mixed guarded commit lost initial custody".into());
        }
        for entry in &mut *self.entries {
            if mode == Mode::Rearm {
                // Every selected Prefix and paired proof passed before any
                // reset. Only now transfer private arena custody for the next
                // run; no signal is reset by this bank transaction.
                entry.pair.finish_rearm(next)?;
            } else {
                entry.pair.phase = Phase::Ready;
            }
        }
        Ok(())
    }

    fn quarantine(&mut self) {
        for entry in &mut *self.entries {
            // Prefix has no Poisoned variant. Submitted plus a poisoned Group
            // cannot be mistaken for a reusable Ready/Completed activation.
            for prefix in &mut entry.prefixes {
                prefix.activation = prefix_state::Activation::Submitted;
            }
            entry.pair.phase = Phase::Poisoned;
            for owner in &mut entry.pair.owners {
                owner.poison(self.group);
            }
        }
        poison_group(self.group);
    }
}

fn with_bank(
    group: &mut Gfx950EngineeringPeerGroupV1,
    entries: &mut [Entry<'_>],
    timeout_ms: u32,
    mode: Mode,
) -> Result<u64> {
    let count = entries.len();
    let mut backend = Native {
        group,
        entries,
        prefixes: Vec::new(),
        all_ids: BTreeSet::new(),
        owner_ids: BTreeSet::new(),
        arena_ids: BTreeSet::new(),
    };
    transact(&mut backend, count, mode, timeout_ms, None)
}

/// Validate fresh generation-one owners without stores or reusable proof output.
pub(in super::super::super) fn initial_bank(
    group: &mut Gfx950EngineeringPeerGroupV1,
    entries: &mut [Entry<'_>],
    timeout_ms: u32,
) -> Result<u64> {
    with_bank(group, entries, timeout_ms, Mode::Initial)
}

/// Reset the whole selected bank, or quarantine it after any failure/unwind.
/// Partial stores are not rolled back. Arenas remain retained until Group close.
///
/// # Safety
/// Own the monotone model/bank/layer ledger for these exact typed entries and
/// permanently retire all prior Prefix commands and references. No outside
/// producer or consumer may access owner storage during this call, and no prior
/// command may access it afterward. New uses require the new committed generation.
/// A terminal Prefix array alone is not proof of these obligations. Paired
/// command retirement is independently rechecked from private sealed custody.
pub(in super::super::super) unsafe fn rearm_bank(
    group: &mut Gfx950EngineeringPeerGroupV1,
    entries: &mut [Entry<'_>],
    timeout_ms: u32,
) -> Result<u64> {
    with_bank(group, entries, timeout_ms, Mode::Rearm)
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_mixed_bank_v1_tests.rs"]
mod tests;
