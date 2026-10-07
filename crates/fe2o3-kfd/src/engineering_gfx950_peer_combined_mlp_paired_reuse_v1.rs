//! Consuming reuse of one sealed pair, never a global arena pool.
use super::*;

fn generation_gate(old: u64, next: u64, current: u64) -> Result<()> {
    if old == 0 || old.checked_add(1) != Some(next) || current != next {
        return Err("paired arena reuse generation mismatch".into());
    }
    Ok(())
}

fn consumed_gate(sealed: [(u64, u64); 2], observed: [(u64, u64); 2]) -> Result<()> {
    for rank in 0..2 {
        if sealed[rank].0 < PACKETS as u64
            || sealed[rank].1 > sealed[rank].0
            || observed[rank].1 < sealed[rank].0
            || observed[rank].1 > observed[rank].0
        {
            return Err("paired arena reuse requires actually consumed old packets".into());
        }
    }
    Ok(())
}

/// Created only by committing a complete, whole-set owner rearm transaction.
/// It has no public snapshot constructor, Clone, fallback allocator, or pool.
pub(in super::super::super) struct Reusable {
    retired: Retired,
    old: u64,
    next: u64,
}

trait ReuseBackend {
    fn now(&mut self) -> Instant;
    fn validate_old(&mut self) -> Result<()>;
    fn write_kernargs(&mut self, rank: usize) -> Result<()>;
    fn reset_signal(&mut self, rank: usize, slot: usize) -> Result<()>;
    fn validate_new(&mut self) -> Result<()>;
    fn quarantine(&mut self);
}

struct Custody<'a, B: ReuseBackend> {
    backend: &'a mut B,
    committed: bool,
}

impl<B: ReuseBackend> Drop for Custody<'_, B> {
    fn drop(&mut self) {
        if !self.committed {
            self.backend.quarantine();
        }
    }
}

fn recycle(
    backend: &mut impl ReuseBackend,
    old: u64,
    next: u64,
    current: u64,
    until: Instant,
) -> Result<()> {
    let mut custody = Custody {
        backend,
        committed: false,
    };
    generation_gate(old, next, current)?;
    deadline_check(custody.backend.now(), until)?;
    // Both complete batches, all ten signals, identities, and the complete
    // kernarg shape are checked before any mutation. Recheck after suffix writes
    // and before the first signal reset; no old peer barrier may still read it.
    custody.backend.validate_old()?;
    deadline_check(custody.backend.now(), until)?;
    for rank in 0..2 {
        custody.backend.write_kernargs(rank)?;
        deadline_check(custody.backend.now(), until)?;
    }
    custody.backend.validate_old()?;
    deadline_check(custody.backend.now(), until)?;
    for rank in 0..2 {
        for slot in 0..PACKETS {
            custody.backend.reset_signal(rank, slot)?;
            deadline_check(custody.backend.now(), until)?;
        }
    }
    custody.backend.validate_new()?;
    deadline_check(custody.backend.now(), until)?;
    custody.committed = true;
    Ok(())
}

struct NativeReuse<'a, 'window> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    proof: &'a mut Retired,
    prepared: &'a [[PreparedDispatch; 4]; 2],
    until: Instant,
    currentness: &'a mut scoped_currentness::Currentness<'window>,
}

impl ReuseBackend for NativeReuse<'_, '_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn validate_old(&mut self) -> Result<()> {
        if self
            .prepared
            .iter()
            .flatten()
            .any(|p| p.bytes.len() > MAX_KERNARG_BYTES_V1 as usize)
        {
            return Err("paired arena reuse kernarg shape".into());
        }
        self.proof
            .recheck_currentness(self.group, self.until, self.currentness)?;
        consumed_gate(self.proof.sealed, self.proof.previous)
    }

    fn write_kernargs(&mut self, rank: usize) -> Result<()> {
        let arena = &self.proof.staged.arenas[rank];
        arena.check(self.group)?;
        let context = &mut self.group.contexts[rank];
        let resource = context
            .buffers
            .get_mut(&arena.local)
            .ok_or("reused paired arena absent")?;
        let values = self.prepared[rank].each_ref().map(|p| p.bytes.as_slice());
        // SAFETY: consuming private retirement custody plus actual queue reads
        // prove no old packet/kernel/barrier can access these suffix slots.
        // Both ranks and immutable bindings were checked before the first write.
        unsafe {
            Backend::rewrite_paired_kernargs_quiescent_v1(
                &mut resource.mapping,
                ARENA_BYTES,
                values,
            )
        }
        .map_err(explain)
    }

    fn reset_signal(&mut self, rank: usize, slot: usize) -> Result<()> {
        let arena = &self.proof.staged.arenas[rank];
        let resource = self.group.contexts[rank]
            .buffers
            .get_mut(&arena.local)
            .ok_or("reused paired signal arena absent")?;
        Backend::reset_completion_signal_release(&mut resource.mapping, ARENA_BYTES, slot as u32)
            .map_err(explain)
    }

    fn validate_new(&mut self) -> Result<()> {
        self.group.require_active()?;
        self.proof.staged.check(self.group)?;
        self.currentness.idle_group(self.group)?;
        if self.proof.staged.read_signals(self.group)? != [[1; PACKETS]; 2] {
            return Err("paired arena reuse pending readback".into());
        }
        Ok(())
    }

    fn quarantine(&mut self) {
        self.proof.staged.poisoned = true;
        self.group.poisoned = true;
        for context in &mut self.group.contexts {
            context.ordered_batch_poisoned = true;
        }
    }
}

impl Reusable {
    pub(in super::super::super) fn require_generation(&self, current: u64) -> Result<()> {
        generation_gate(self.old, self.next, current)?;
        self.retired.staged.require_retired()
    }

    pub(in super::super::super) fn new(retired: Retired, old: u64, next: u64) -> Result<Self> {
        generation_gate(old, next, next)?;
        retired.staged.require_retired()?;
        Ok(Self { retired, old, next })
    }

    pub(in super::super::super) fn prepare(
        mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        prepared: [[PreparedDispatch; 4]; 2],
        generation: u64,
        until: Instant,
    ) -> Result<Staged> {
        self.prepare_currentness(
            group,
            prepared,
            generation,
            until,
            &mut scoped_currentness::Currentness::Full,
        )
    }

    pub(in super::super::super) fn prepare_currentness(
        mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        prepared: [[PreparedDispatch; 4]; 2],
        generation: u64,
        until: Instant,
        currentness: &mut scoped_currentness::Currentness<'_>,
    ) -> Result<Staged> {
        recycle(
            &mut NativeReuse {
                group,
                proof: &mut self.retired,
                prepared: &prepared,
                until,
                currentness,
            },
            self.old,
            self.next,
            generation,
            until,
        )?;
        // New reservations and batches; neither old publication state nor old
        // logical completion/read credit survives as the next batch's state.
        Staged::from_arenas(group, self.retired.staged.arenas, prepared)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_reuse_v1_tests.rs"]
mod tests;
