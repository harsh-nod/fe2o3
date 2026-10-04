//! Owner-only typed V2 state; no ordinary buffer or public pointer conversion.

use super::super::wave_mlp_tiles_v2 as profile;
use super::*;

pub(super) const STATE_BYTES: usize = 2192;
pub(super) const STATE_WORDS: usize = 548;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Activation {
    Allocated,
    Initialized,
    Ready,
    Submitted,
    Completed,
}

/// A single owner's 548 genuine AtomicU32 values, retained until group close.
/// Neither a generic buffer token nor a pointer escapes this type. A successful
/// resident launch consumes Ready; only explicit unsafe quiescent rearm restores it.
///
/// ```compile_fail
/// let _ = fe2o3_kfd::Gfx950EngineeringPeerWaveMlpTilesStateV2 { buffer: todo!() };
/// ```
///
/// ```compile_fail
/// fn not_v2(old: fe2o3_kfd::Gfx950EngineeringPeerWaveMlpStateV1) {
///     let _: fe2o3_kfd::Gfx950EngineeringPeerWaveMlpTilesStateV2 = old;
/// }
/// ```
///
/// ```compile_fail
/// fn no_generic_pointer(state: &fe2o3_kfd::Gfx950EngineeringPeerWaveMlpTilesStateV2)
///     -> fe2o3_kfd::Gfx950EngineeringPeerPointerV1 {
///     state.pointer()
/// }
/// ```
#[derive(Debug)]
pub struct Gfx950EngineeringPeerWaveMlpTilesStateV2 {
    pub(super) buffer: Gfx950EngineeringPeerBufferV1,
    pub(super) activation: Activation,
}

impl Gfx950EngineeringPeerWaveMlpTilesStateV2 {
    pub const fn owner_rank(&self) -> usize {
        self.buffer.owner
    }

    pub(super) fn pointer(&self) -> Gfx950EngineeringPeerPointerV1 {
        self.buffer
            .pointer(80, 0, STATE_BYTES as u64, BufferAccessV1::ReadWrite)
    }

    pub(super) fn local_id(&self, group: &Gfx950EngineeringPeerGroupV1) -> Result<u64> {
        if self.owner_rank() >= group.contexts.len() {
            return Err("MLP tiles V2 state owner outside group".into());
        }
        let record = group.validate_token(self.buffer)?;
        validate_state_record(self, record)?;
        let allocation = group.contexts[self.owner_rank()]
            .buffers
            .get(&record.local_id)
            .ok_or("missing MLP tiles V2 local allocation")?;
        if allocation.requested != STATE_BYTES {
            return Err("MLP tiles V2 exact requested extent changed".into());
        }
        Ok(record.local_id)
    }
}

fn validate_state_record(
    state: &Gfx950EngineeringPeerWaveMlpTilesStateV2,
    record: &BufferRecord,
) -> Result<()> {
    if record.token != state.buffer
        || state.buffer.bytes != STATE_BYTES as u64
        || record.kind != BufferKind::WaveMlpTilesStateV2
        || record.mapping.phase != Phase::PeersMapped
        || !record.mapping.peers.is_empty()
        || record.mapping.mapped != 0
        || record.mapping.unmapped != 0
    {
        return Err("state is not owner-only MLP tiles V2".into());
    }
    Ok(())
}

fn allocation_ids(
    world: usize,
    owner: usize,
    total: usize,
    local: usize,
    id: u64,
    local_id: u64,
) -> Result<(u64, u64)> {
    if owner >= world || total >= group_allocation_limit(world)? || local >= MAX_ALLOCATIONS {
        return Err("MLP tiles V2 allocation bounds".into());
    }
    Ok((
        id.checked_add(1).ok_or("peer buffer identity exhausted")?,
        local_id
            .checked_add(1)
            .ok_or("local buffer identity exhausted")?,
    ))
}

fn begin_initialization(state: &mut Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<()> {
    if state.activation != Activation::Allocated {
        return Err("MLP tiles V2 late or repeated atomic construction".into());
    }
    state.activation = Activation::Initialized;
    Ok(())
}

trait StateBackend {
    fn check(&mut self) -> Result<()>;
    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveMlpTilesStateV2>;
    fn initialize(&mut self, state: &mut Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<()>;
    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveMlpTilesStateV2,
    ) -> Result<[u32; STATE_WORDS]>;
    fn rearm(&mut self, state: &Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<()>;
}

fn allocate_initialized(
    backend: &mut impl StateBackend,
) -> Result<Gfx950EngineeringPeerWaveMlpTilesStateV2> {
    backend.check()?;
    let mut state = backend.allocate()?;
    backend.initialize(&mut state)?;
    if state.activation != Activation::Initialized
        || backend.observe(&state)? != profile::INITIAL_STATE
    {
        return Err("MLP tiles V2 initialization mismatch".into());
    }
    backend.check()?;
    state.activation = Activation::Ready;
    Ok(state)
}

fn observe_idle(
    backend: &mut impl StateBackend,
    state: &Gfx950EngineeringPeerWaveMlpTilesStateV2,
) -> Result<[u32; STATE_WORDS]> {
    backend.check()?;
    let words = backend.observe(state)?;
    backend.check()?;
    Ok(words)
}

fn rearm_terminal(
    backend: &mut impl StateBackend,
    state: &mut Gfx950EngineeringPeerWaveMlpTilesStateV2,
    expected: &[u32; STATE_WORDS],
) -> Result<()> {
    if state.activation != Activation::Completed {
        return Err("MLP tiles V2 rearm requires completed resident activation".into());
    }
    profile::validate_final_state(*expected)?;
    backend.check()?;
    if backend.observe(state)? != *expected {
        return Err("MLP tiles V2 terminal words changed before rearm".into());
    }
    // The external token must not look completed if any store/readback fails.
    state.activation = Activation::Submitted;
    backend.rearm(state)?;
    if backend.observe(state)? != profile::INITIAL_STATE {
        return Err("MLP tiles V2 rearm readback mismatch".into());
    }
    backend.check()?;
    state.activation = Activation::Ready;
    Ok(())
}

struct NativeState<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    owner: usize,
}

impl NativeState<'_> {
    fn local_id(&self, state: &Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<u64> {
        if state.owner_rank() != self.owner {
            return Err("MLP tiles V2 owner mismatch".into());
        }
        state.local_id(self.group)
    }
}

impl StateBackend for NativeState<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveMlpTilesStateV2> {
        if self.owner >= self.group.contexts.len() {
            return Err("MLP tiles V2 allocation owner".into());
        }
        let id = self.group.next_buffer;
        let local_id = self.group.contexts[self.owner].next_buffer;
        let (next_id, next_local_id) = allocation_ids(
            self.group.contexts.len(),
            self.owner,
            self.group.buffers.len(),
            self.group.contexts[self.owner].buffers.len(),
            id,
            local_id,
        )?;
        self.group.next_buffer = next_id;
        let context = &mut self.group.contexts[self.owner];
        context.next_buffer = next_local_id;
        let allocation = context.allocate_resource(
            STATE_BYTES,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
            |_| Ok(()),
        )?;
        context.buffers.insert(local_id, allocation);
        let token = Gfx950EngineeringPeerBufferV1 {
            group: self.group.incarnation,
            id,
            owner: self.owner,
            bytes: STATE_BYTES as u64,
        };
        self.group.buffers.insert(
            id,
            BufferRecord {
                token,
                local_id,
                mapping: PeerMapping::new(Vec::new())?,
                kind: BufferKind::WaveMlpTilesStateV2,
            },
        );
        self.group
            .buffers
            .get_mut(&id)
            .ok_or("missing new V2 state")?
            .mapping
            .map(&mut NativeTransaction {
                contexts: &mut self.group.contexts,
                shared_full_currentness: self.group.shared_full_currentness,
                owner: self.owner,
                local_id,
            })?;
        Ok(Gfx950EngineeringPeerWaveMlpTilesStateV2 {
            buffer: token,
            activation: Activation::Allocated,
        })
    }

    fn initialize(&mut self, state: &mut Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<()> {
        let local_id = self.local_id(state)?;
        begin_initialization(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing V2 state at initialize")?;
        Backend::initialize_engineering_wave_mlp_tile_state_v2(
            &mut allocation.mapping,
            allocation.requested,
        )
        .map_err(explain)
    }

    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveMlpTilesStateV2,
    ) -> Result<[u32; STATE_WORDS]> {
        let local_id = self.local_id(state)?;
        if state.activation == Activation::Allocated {
            return Err("MLP tiles V2 atomic lifetime not initialized".into());
        }
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing V2 state at acquire")?;
        Backend::observe_engineering_wave_mlp_tile_state_v2(
            &mut allocation.mapping,
            allocation.requested,
        )
        .map_err(explain)
    }

    fn rearm(&mut self, state: &Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<()> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing V2 state at rearm")?;
        Backend::rearm_engineering_wave_mlp_tile_state_v2(
            &mut allocation.mapping,
            allocation.requested,
        )
        .map_err(explain)
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Allocate exactly 2192 coherent bytes and initialize all 548 real atomics.
    /// Failure poisons the group and retains its underlying allocation records.
    pub fn allocate_wave_mlp_tiles_state_v2(
        &mut self,
        owner: usize,
    ) -> Result<Gfx950EngineeringPeerWaveMlpTilesStateV2> {
        self.require_active()?;
        let result = allocate_initialized(&mut NativeState { group: self, owner });
        self.finish(result)
    }

    /// Acquire observations bracketed by all-participant idle/currentness checks.
    /// This does not grant completion, another activation, or context teardown.
    pub fn observe_wave_mlp_tiles_state_v2(
        &mut self,
        state: &Gfx950EngineeringPeerWaveMlpTilesStateV2,
    ) -> Result<[u32; STATE_WORDS]> {
        self.require_active()?;
        let result = observe_idle(
            &mut NativeState {
                group: self,
                owner: state.owner_rank(),
            },
            state,
        );
        self.finish(result)
    }

    /// Store the original epoch1 values into already-constructed idle atomics.
    /// # Safety
    /// Own a separately validated monotone-generation ledger, commit the prior
    /// completed use, and permanently retire all its commands and references.
    /// No use may overlap this call. An acquired terminal array alone does not
    /// establish those premises. This is not production reuse authority.
    pub unsafe fn rearm_wave_mlp_tiles_state_v2(
        &mut self,
        state: &mut Gfx950EngineeringPeerWaveMlpTilesStateV2,
        expected: &[u32; STATE_WORDS],
    ) -> Result<()> {
        self.require_active()?;
        let result = rearm_terminal(
            &mut NativeState {
                group: self,
                owner: state.owner_rank(),
            },
            state,
            expected,
        );
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_mlp_tiles_state_v2_tests.rs"]
mod tests;
