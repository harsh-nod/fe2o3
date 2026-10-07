//! Private proposal: one genuine 552-atomic owner, not an old StateV2 token.
use super::super::wave_mlp_tiles_v2 as profile;
use super::*;

#[path = "engineering_gfx950_peer_combined_mlp_paired_v1.rs"]
pub(super) mod paired;

pub(super) const PREFIX_WORDS: usize = 548;
pub(super) const PREFIX_BYTES: usize = 2192;
pub(super) const GUARD_OFFSET: usize = PREFIX_BYTES;
pub(super) const GUARD_BYTES: usize = 16;
pub(super) const COMBINED_BYTES: usize = PREFIX_BYTES + GUARD_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Activation {
    Allocated,
    Initialized,
    Ready,
    Submitted,
    Completed,
    Rearming,
    Poisoned,
}

/// Neither this non-Clone owner nor its views expose a generic buffer token.
/// The group retains the backing even if this value is dropped.
#[derive(Debug)]
pub(super) struct CombinedMlpStateV1 {
    buffer: Gfx950EngineeringPeerBufferV1,
    activation: Activation,
    generation: u64,
}

/// Data only. A snapshot never supplies completion, quiescence or reuse rights.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CombinedMlpSnapshotV1 {
    pub(super) prefix: [u32; PREFIX_WORDS],
    pub(super) guard: [u32; 4],
}

/// Private preparation views borrow the owner; no old 2,192-byte token is made.
pub(super) struct CombinedMlpRegionsV1<'a> {
    state: &'a CombinedMlpStateV1,
}

impl CombinedMlpRegionsV1<'_> {
    pub(super) fn mlp_prefix(&self, rank: usize) -> Result<Gfx950EngineeringPeerPointerV1> {
        if rank != self.state.buffer.owner {
            return Err("combined MLP prefix is owner-write only".into());
        }
        Ok(self
            .state
            .buffer
            .pointer(80, 0, PREFIX_BYTES as u64, BufferAccessV1::ReadWrite))
    }

    pub(super) fn validator(&self, rank: usize) -> Result<Gfx950EngineeringPeerPointerV1> {
        if rank != self.state.buffer.owner {
            return Err("combined MLP validator is owner-write only".into());
        }
        Ok(self
            .state
            .buffer
            .pointer(0, 0, COMBINED_BYTES as u64, BufferAccessV1::ReadWrite))
    }

    pub(super) fn r2_guard(
        &self,
        rank: usize,
        kernarg_offset: u32,
    ) -> Result<Gfx950EngineeringPeerPointerV1> {
        if rank >= 2 || !matches!(kernarg_offset, 64 | 80) {
            return Err("combined MLP R2 rank or guard argument".into());
        }
        Ok(self.state.buffer.pointer(
            kernarg_offset,
            GUARD_OFFSET as u64,
            GUARD_BYTES as u64,
            BufferAccessV1::Read,
        ))
    }
}

/// Enforce the narrow regions again at actual pointer preparation, not merely
/// in the convenience view. Existing buffer kinds keep their original policy.
pub(super) fn validate_pointer(
    record: &BufferRecord,
    pointer: &Gfx950EngineeringPeerPointerV1,
    rank: usize,
) -> Result<()> {
    if record.kind != BufferKind::CombinedMlpStateV1 {
        return Ok(());
    }
    let suffix = pointer.buffer_offset == GUARD_OFFSET as u64
        && pointer.extent_bytes == GUARD_BYTES as u64
        && pointer.access == BufferAccessV1::Read
        && matches!(pointer.kernarg_offset, 64 | 80);
    let owner_region = rank == record.token.owner
        && pointer.buffer_offset == 0
        && pointer.access == BufferAccessV1::ReadWrite
        && ((pointer.extent_bytes == PREFIX_BYTES as u64 && pointer.kernarg_offset == 80)
            || (pointer.extent_bytes == COMBINED_BYTES as u64 && pointer.kernarg_offset == 0));
    if rank >= 2 || record.token.bytes != COMBINED_BYTES as u64 || !(suffix || owner_region) {
        return Err("combined MLP pointer outside private typed regions".into());
    }
    Ok(())
}

fn guard_words(generation: u64, verdict: u32) -> [u32; 4] {
    [generation as u32, (generation >> 32) as u32, verdict, 0]
}

fn require_activation(actual: Activation, expected: Activation) -> Result<()> {
    if actual != expected {
        return Err("combined MLP activation mismatch".into());
    }
    Ok(())
}

fn require_initial(snapshot: &CombinedMlpSnapshotV1, generation: u64) -> Result<()> {
    if generation == 0
        || snapshot.prefix != profile::INITIAL_STATE
        || snapshot.guard != guard_words(generation, 0)
    {
        return Err("combined MLP initial readback mismatch".into());
    }
    Ok(())
}

fn require_terminal(snapshot: &CombinedMlpSnapshotV1, generation: u64) -> Result<()> {
    profile::validate_final_state(snapshot.prefix)?;
    if generation == 0 || snapshot.guard != guard_words(generation, 1) {
        return Err("combined MLP terminal guard is not current Valid".into());
    }
    Ok(())
}

fn require_rearm(activation: Activation, previous: u64, next: u64) -> Result<()> {
    require_activation(activation, Activation::Completed)?;
    if previous == 0 || previous.checked_add(1) != Some(next) {
        return Err("combined MLP rearm requires exact next nonzero generation".into());
    }
    Ok(())
}

impl CombinedMlpStateV1 {
    pub(super) fn owner_rank(&self) -> usize {
        self.buffer.owner
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    fn local_id(&self, group: &Gfx950EngineeringPeerGroupV1) -> Result<u64> {
        group.require_active()?;
        if group.contexts.len() != 2 || self.buffer.owner >= 2 || self.generation == 0 {
            return Err("combined MLP owner/group/generation".into());
        }
        let record = group.validate_token(self.buffer)?;
        let peer_id = group.contexts[1 - self.buffer.owner].backend.gpu_id();
        if record.kind != BufferKind::CombinedMlpStateV1
            || self.buffer.bytes != COMBINED_BYTES as u64
            || record.mapping.peers.as_slice() != [peer_id]
            || record.mapping.mapped != 1
            || record.mapping.unmapped != 0
        {
            return Err("combined MLP typed peer custody".into());
        }
        let allocation = group.contexts[self.buffer.owner]
            .buffers
            .get(&record.local_id)
            .ok_or("combined MLP allocation missing")?;
        if allocation.requested != COMBINED_BYTES {
            return Err("combined MLP exact requested extent changed".into());
        }
        Ok(record.local_id)
    }

    pub(super) fn regions<'a>(
        &'a self,
        group: &Gfx950EngineeringPeerGroupV1,
    ) -> Result<CombinedMlpRegionsV1<'a>> {
        self.local_id(group)?;
        require_activation(self.activation, Activation::Ready)?;
        Ok(CombinedMlpRegionsV1 { state: self })
    }

    fn observe(&self, group: &mut Gfx950EngineeringPeerGroupV1) -> Result<CombinedMlpSnapshotV1> {
        let local_id = self.local_id(group)?;
        if matches!(
            self.activation,
            Activation::Allocated | Activation::Poisoned
        ) {
            return Err("combined MLP observation requires initialized healthy atomics".into());
        }
        let allocation = group.contexts[self.buffer.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("combined MLP observation allocation missing")?;
        // SAFETY: one-shot initialization has completed; the group owns storage.
        let (prefix, guard) = unsafe {
            Backend::observe_combined_mlp_state_v1(&mut allocation.mapping, allocation.requested)
        }
        .map_err(explain)?;
        Ok(CombinedMlpSnapshotV1 { prefix, guard })
    }

    pub(super) fn poison(&mut self, group: &mut Gfx950EngineeringPeerGroupV1) {
        self.activation = Activation::Poisoned;
        group.poisoned = true;
    }

    fn outcome<T>(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        result: Result<T>,
    ) -> Result<T> {
        if result.is_err() {
            self.poison(group);
        }
        result
    }

    /// Consume Ready after all private packet preparations and before either
    /// queue publication. The coordinator must poison both owners on any later
    /// failure, including a failure to submit the other rank.
    pub(super) fn submit(&mut self, group: &mut Gfx950EngineeringPeerGroupV1) -> Result<()> {
        self.submit_currentness(group, &mut scoped_currentness::Currentness::Full)
    }

    pub(super) fn submit_currentness(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        currentness: &mut scoped_currentness::Currentness<'_>,
    ) -> Result<()> {
        let result = (|| {
            require_activation(self.activation, Activation::Ready)?;
            currentness.idle_group(group)?;
            let observed = self.observe(group)?;
            require_initial(&observed, self.generation)?;
            self.activation = Activation::Submitted;
            Ok(())
        })();
        self.outcome(group, result)
    }

    /// # Safety
    /// The future coordinator must prove BOTH closed batches were published,
    /// all ten actual signals completed, both guards are current, no fault or
    /// deadline violation occurred, and all old users are quiescent. This
    /// method supplies only this owner's terminal readback, not those proofs.
    pub(super) unsafe fn complete_quiescent(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
    ) -> Result<CombinedMlpSnapshotV1> {
        // SAFETY: unchanged legacy currentness and quiescence contract.
        unsafe {
            self.complete_quiescent_currentness(group, &mut scoped_currentness::Currentness::Full)
        }
    }

    pub(super) unsafe fn complete_quiescent_currentness(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        currentness: &mut scoped_currentness::Currentness<'_>,
    ) -> Result<CombinedMlpSnapshotV1> {
        let result = (|| {
            require_activation(self.activation, Activation::Submitted)?;
            currentness.idle_group(group)?;
            let observed = self.observe(group)?;
            require_terminal(&observed, self.generation)?;
            currentness.idle_group(group)?;
            self.activation = Activation::Completed;
            Ok(observed)
        })();
        self.outcome(group, result)
    }

    /// # Safety
    /// The future coordinator has completed the paired terminal gate, entered
    /// Busy from healthy Idle, and established whole-group quiescence. No old
    /// queued access or R2 peer read remains; no new packet is published. An
    /// expected snapshot alone never establishes these obligations.
    pub(super) unsafe fn rearm_quiescent(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        expected: &CombinedMlpSnapshotV1,
        generation: u64,
    ) -> Result<()> {
        let result = (|| {
            require_rearm(self.activation, self.generation, generation)?;
            require_terminal(expected, self.generation)?;
            check_contexts(&mut group.contexts, group.shared_full_currentness)?;
            if self.observe(group)? != *expected {
                return Err("combined MLP terminal words changed before rearm".into());
            }
            let local_id = self.local_id(group)?;
            self.activation = Activation::Rearming;
            let allocation = group.contexts[self.buffer.owner]
                .buffers
                .get_mut(&local_id)
                .ok_or("combined MLP rearm allocation missing")?;
            // SAFETY: caller established the paired quiescent generation fence.
            unsafe {
                Backend::rearm_combined_mlp_state_v1(
                    &mut allocation.mapping,
                    allocation.requested,
                    &profile::INITIAL_STATE,
                    generation,
                )
            }
            .map_err(explain)?;
            let observed = self.observe(group)?;
            require_initial(&observed, generation)?;
            check_contexts(&mut group.contexts, group.shared_full_currentness)?;
            self.generation = generation;
            self.activation = Activation::Ready;
            Ok(())
        })();
        self.outcome(group, result)
    }
}

/// Private allocation entry; no worker/profile route calls this proposal yet.
pub(super) fn allocate(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owner: usize,
) -> Result<CombinedMlpStateV1> {
    group.require_active()?;
    let result = (|| {
        if group.contexts.len() != 2
            || owner >= 2
            || group.buffers.len() >= group_allocation_limit(2)?
            || group.contexts[owner].buffers.len() >= MAX_ALLOCATIONS
        {
            return Err("combined MLP allocation bounds".into());
        }
        check_contexts(&mut group.contexts, group.shared_full_currentness)?;
        let id = group.next_buffer;
        let local_id = group.contexts[owner].next_buffer;
        let next_id = id
            .checked_add(1)
            .ok_or("combined MLP group identity exhausted")?;
        let next_local = local_id
            .checked_add(1)
            .ok_or("combined MLP local identity exhausted")?;
        let peer_id = group.contexts[1 - owner].backend.gpu_id();
        let peer_aperture = group.contexts[1 - owner].backend.gpuvm_aperture();
        let mapping = PeerMapping::new(vec![peer_id])?;
        group.next_buffer = next_id;
        group.contexts[owner].next_buffer = next_local;
        let allocation = group.contexts[owner].allocate_resource(
            COMBINED_BYTES,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
            |_| Ok(()),
        )?;
        let base = allocation.va;
        let backing = allocation.backing;
        group.contexts[owner].buffers.insert(local_id, allocation);
        let buffer = Gfx950EngineeringPeerBufferV1 {
            group: group.incarnation,
            id,
            owner,
            bytes: COMBINED_BYTES as u64,
        };
        // Retain custody before any later validation, mapping or initialization
        // can fail. Failed groups quarantine rather than retry/free resources.
        group.buffers.insert(
            id,
            BufferRecord {
                token: buffer,
                local_id,
                mapping,
                kind: BufferKind::CombinedMlpStateV1,
            },
        );
        let end = base
            .checked_add(backing.checked_sub(1).ok_or("combined MLP empty backing")? as u64)
            .ok_or("combined MLP peer VA overflow")?;
        if base < peer_aperture.base() || end > peer_aperture.limit() {
            return Err("combined MLP backing outside peer aperture".into());
        }
        group
            .buffers
            .get_mut(&id)
            .ok_or("combined MLP new record missing")?
            .mapping
            .map(&mut NativeTransaction {
                contexts: &mut group.contexts,
                shared_full_currentness: group.shared_full_currentness,
                owner,
                local_id,
            })?;
        let mut state = CombinedMlpStateV1 {
            buffer,
            activation: Activation::Allocated,
            generation: 1,
        };
        state.local_id(group)?;
        state.activation = Activation::Initialized;
        let allocation = group.contexts[owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("combined MLP initialize allocation missing")?;
        // SAFETY: fresh private allocation, never published, one-shot lifetime.
        unsafe {
            Backend::initialize_combined_mlp_state_v1(
                &mut allocation.mapping,
                allocation.requested,
                &profile::INITIAL_STATE,
                1,
            )
        }
        .map_err(explain)?;
        let observed = state.observe(group)?;
        require_initial(&observed, 1)?;
        check_contexts(&mut group.contexts, group.shared_full_currentness)?;
        state.activation = Activation::Ready;
        Ok(state)
    })();
    group.finish(result)
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_state_v1_tests.rs"]
mod tests;
