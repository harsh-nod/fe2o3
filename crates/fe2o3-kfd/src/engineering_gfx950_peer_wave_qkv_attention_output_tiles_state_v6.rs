//! Owner-only typed V6 state; no ordinary buffer or public pointer conversion.

use super::super::wave_qkv_attention_output_tiles_v6 as profile;
use super::*;

pub(super) const STATE_BYTES: usize = 1136;
pub(super) const STATE_WORDS: usize = 284;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Activation {
    Allocated,
    Initialized,
    Ready,
    Submitted,
    Completed,
}

/// A single owner's 284 genuine AtomicU32 values, retained until group close.
/// Neither a generic buffer token nor a pointer escapes this type. A successful
/// resident launch consumes Ready; only explicit unsafe quiescent rearm restores it.
///
/// ```compile_fail
/// let _ = fe2o3_kfd::Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 { buffer: todo!() };
/// ```
///
/// ```compile_fail
/// fn not_v6(old: fe2o3_kfd::Gfx950EngineeringPeerWaveOutputStateV5) {
///     let _: fe2o3_kfd::Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 = old;
/// }
/// ```
///
/// ```compile_fail
/// fn no_generic_pointer(state: &fe2o3_kfd::Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6)
///     -> fe2o3_kfd::Gfx950EngineeringPeerPointerV1 {
///     state.pointer()
/// }
/// ```
#[derive(Debug)]
pub struct Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 {
    pub(super) buffer: Gfx950EngineeringPeerBufferV1,
    pub(super) activation: Activation,
}

impl Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 {
    pub const fn owner_rank(&self) -> usize {
        self.buffer.owner
    }

    pub(super) fn pointer(&self) -> Gfx950EngineeringPeerPointerV1 {
        self.buffer
            .pointer(112, 0, STATE_BYTES as u64, BufferAccessV1::ReadWrite)
    }

    pub(super) fn local_id(&self, group: &Gfx950EngineeringPeerGroupV1) -> Result<u64> {
        if self.owner_rank() >= group.contexts.len() {
            return Err("prefix tiles V6 state owner outside group".into());
        }
        let record = group.validate_token(self.buffer)?;
        validate_state_record(self, record)?;
        let allocation = group.contexts[self.owner_rank()]
            .buffers
            .get(&record.local_id)
            .ok_or("missing prefix tiles V6 local allocation")?;
        if allocation.requested != STATE_BYTES {
            return Err("prefix tiles V6 exact requested extent changed".into());
        }
        Ok(record.local_id)
    }
}

fn validate_state_record(
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    record: &BufferRecord,
) -> Result<()> {
    if record.token != state.buffer
        || state.buffer.bytes != STATE_BYTES as u64
        || record.kind != BufferKind::WaveQkvAttentionOutputTilesStateV6
        || record.mapping.phase != Phase::PeersMapped
        || !record.mapping.peers.is_empty()
        || record.mapping.mapped != 0
        || record.mapping.unmapped != 0
    {
        return Err("state is not owner-only prefix tiles V6".into());
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
        return Err("prefix tiles V6 allocation bounds".into());
    }
    Ok((
        id.checked_add(1).ok_or("peer buffer identity exhausted")?,
        local_id
            .checked_add(1)
            .ok_or("local buffer identity exhausted")?,
    ))
}

fn begin_initialization(
    state: &mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<()> {
    if state.activation != Activation::Allocated {
        return Err("prefix tiles V6 late or repeated atomic construction".into());
    }
    state.activation = Activation::Initialized;
    Ok(())
}

trait StateBackend {
    fn check(&mut self) -> Result<()>;
    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6>;
    fn initialize(
        &mut self,
        state: &mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<()>;
    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<[u32; STATE_WORDS]>;
    fn rearm(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<()>;
}

fn allocate_initialized(
    backend: &mut impl StateBackend,
) -> Result<Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6> {
    backend.check()?;
    let mut state = backend.allocate()?;
    backend.initialize(&mut state)?;
    if state.activation != Activation::Initialized
        || backend.observe(&state)? != profile::INITIAL_STATE
    {
        return Err("prefix tiles V6 initialization mismatch".into());
    }
    backend.check()?;
    state.activation = Activation::Ready;
    Ok(state)
}

fn observe_idle(
    backend: &mut impl StateBackend,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<[u32; STATE_WORDS]> {
    backend.check()?;
    let words = backend.observe(state)?;
    backend.check()?;
    Ok(words)
}

fn rearm_terminal(
    backend: &mut impl StateBackend,
    state: &mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    expected: &[u32; STATE_WORDS],
) -> Result<()> {
    if state.activation != Activation::Completed {
        return Err("prefix tiles V6 rearm requires completed resident activation".into());
    }
    profile::validate_final_state(*expected)?;
    backend.check()?;
    if backend.observe(state)? != *expected {
        return Err("prefix tiles V6 terminal words changed before rearm".into());
    }
    // The external token must not look completed if any store/readback fails.
    state.activation = Activation::Submitted;
    backend.rearm(state)?;
    if backend.observe(state)? != profile::INITIAL_STATE {
        return Err("prefix tiles V6 rearm readback mismatch".into());
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
    fn local_id(
        &self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<u64> {
        if state.owner_rank() != self.owner {
            return Err("prefix tiles V6 owner mismatch".into());
        }
        state.local_id(self.group)
    }
}

impl StateBackend for NativeState<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6> {
        if self.owner >= self.group.contexts.len() {
            return Err("prefix tiles V6 allocation owner".into());
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
                kind: BufferKind::WaveQkvAttentionOutputTilesStateV6,
            },
        );
        self.group
            .buffers
            .get_mut(&id)
            .ok_or("missing new V6 state")?
            .mapping
            .map(&mut NativeTransaction {
                contexts: &mut self.group.contexts,
                shared_full_currentness: self.group.shared_full_currentness,
                owner: self.owner,
                local_id,
            })?;
        Ok(Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 {
            buffer: token,
            activation: Activation::Allocated,
        })
    }

    fn initialize(
        &mut self,
        state: &mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<()> {
        let local_id = self.local_id(state)?;
        begin_initialization(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing V6 state at initialize")?;
        Backend::initialize_engineering_wave_qkv_attention_output_tile_state_v6(
            &mut allocation.mapping,
            allocation.requested,
        )
        .map_err(explain)
    }

    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<[u32; STATE_WORDS]> {
        let local_id = self.local_id(state)?;
        if state.activation == Activation::Allocated {
            return Err("prefix tiles V6 atomic lifetime not initialized".into());
        }
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing V6 state at acquire")?;
        Backend::observe_engineering_wave_qkv_attention_output_tile_state_v6(
            &mut allocation.mapping,
            allocation.requested,
        )
        .map_err(explain)
    }

    fn rearm(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<()> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing V6 state at rearm")?;
        Backend::rearm_engineering_wave_qkv_attention_output_tile_state_v6(
            &mut allocation.mapping,
            allocation.requested,
        )
        .map_err(explain)
    }
}

pub(super) fn validate_idle_bank_state(
    group: &Gfx950EngineeringPeerGroupV1,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<()> {
    group.require_active()?;
    if !matches!(
        state.activation,
        Activation::Ready | Activation::Submitted | Activation::Completed
    ) {
        return Err("prefix tiles V6 idle bank observation activation".into());
    }
    validate_state_record(state, group.validate_token(state.buffer)?)?;
    state.local_id(group)?;
    Ok(())
}

/// Observe constructed idle atomics within the separate bounded-bank fence pair.
/// Unlike the resident accessor, this permits a completed activation.
///
/// # Safety
/// The bank coordinator must hold the exclusive group borrow and all mappings
/// across fresh all-participant idle/currentness checks before and after every
/// read. No dispatch, map, release, or rearm may interleave. No snapshot escapes
/// before the trailing check succeeds; any failure must quarantine the group.
pub(super) unsafe fn observe_within_idle_bank_fence(
    group: &mut Gfx950EngineeringPeerGroupV1,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<[u32; STATE_WORDS]> {
    observe_bank_state(group, state)
}

/// # Safety
/// Only the closed scoped bank transaction may call this, holding every typed
/// owner and Group through its participant checkpoints and mandatory full exit.
/// Refusal/unwind must quarantine the entire bank. This does not satisfy the
/// legacy immediate full-fence contract.
pub(super) unsafe fn observe_within_scoped_bank(
    group: &mut Gfx950EngineeringPeerGroupV1,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<[u32; STATE_WORDS]> {
    observe_bank_state(group, state)
}

fn observe_bank_state(
    group: &mut Gfx950EngineeringPeerGroupV1,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<[u32; STATE_WORDS]> {
    group.require_active()?;
    let result = (|| {
        validate_idle_bank_state(group, state)?;
        NativeState {
            group: &mut *group,
            owner: state.owner_rank(),
        }
        .observe(state)
    })();
    group.finish(result)
}

struct ScopedRearmState<'group, 'route, 'window> {
    native: NativeState<'group>,
    currentness: &'route mut scoped_currentness::Currentness<'window>,
}

impl StateBackend for ScopedRearmState<'_, '_, '_> {
    fn check(&mut self) -> Result<()> {
        self.currentness.idle_group(self.native.group)
    }

    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6> {
        Err("scoped bank rearm cannot allocate".into())
    }

    fn initialize(
        &mut self,
        _: &mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<()> {
        Err("scoped bank rearm cannot initialize fresh storage".into())
    }

    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<[u32; STATE_WORDS]> {
        self.native.observe(state)
    }

    fn rearm(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<()> {
        self.native.rearm(state)
    }
}

/// # Safety
/// The closed scoped bank retains permanent old-command retirement and all
/// typed owner custody, validates every bank member before any reset, and
/// quarantines all owners on error/unwind through its final full exit.
pub(super) unsafe fn rearm_within_scoped_bank(
    group: &mut Gfx950EngineeringPeerGroupV1,
    state: &mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    expected: &[u32; STATE_WORDS],
    currentness: &mut scoped_currentness::Currentness<'_>,
) -> Result<()> {
    group.require_active()?;
    let result = rearm_terminal(
        &mut ScopedRearmState {
            native: NativeState {
                group: &mut *group,
                owner: state.owner_rank(),
            },
            currentness,
        },
        state,
        expected,
    );
    group.finish(result)
}

/// Read one private resident state inside its coordinator's full-fence pair.
/// The public observer keeps its own fences; only the paired resident path
/// may use this accessor. No topology snapshot or reuse capability escapes.
///
/// # Safety
/// A fresh successful all-participant idle/currentness fence must precede this
/// read, and another must follow the entire state pair before publication or
/// completed activations escape. Retain the exclusive group borrow and all
/// mappings throughout; no dispatch, map, release, or rearm may interleave.
pub(super) unsafe fn observe_within_resident_fence(
    group: &mut Gfx950EngineeringPeerGroupV1,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<[u32; STATE_WORDS]> {
    observe_resident_state(group, state)
}

/// # Safety
/// Only the closed scoped LayerOperation may call this under its exclusive
/// Group/Prefix/pair custody, bracketed scoped checks and mandatory full exit.
/// This does not satisfy the legacy immediate-full-fence premise.
pub(super) unsafe fn observe_within_scoped_layer(
    group: &mut Gfx950EngineeringPeerGroupV1,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<[u32; STATE_WORDS]> {
    observe_resident_state(group, state)
}

fn observe_resident_state(
    group: &mut Gfx950EngineeringPeerGroupV1,
    state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
) -> Result<[u32; STATE_WORDS]> {
    group.require_active()?;
    let result = (|| {
        if !matches!(state.activation, Activation::Ready | Activation::Submitted) {
            return Err("prefix tiles V6 private resident observation activation".into());
        }
        // Keep the private entry's identity/kind/mapping refusal before storage
        // access. NativeState also checks the owner and exact local allocation.
        validate_state_record(state, group.validate_token(state.buffer)?)?;
        NativeState {
            group: &mut *group,
            owner: state.owner_rank(),
        }
        .observe(state)
    })();
    group.finish(result)
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Allocate exactly 1136 coherent bytes and initialize all 284 real atomics.
    /// Failure poisons the group and retains its underlying allocation records.
    pub fn allocate_wave_qkv_attention_output_tiles_state_v6(
        &mut self,
        owner: usize,
    ) -> Result<Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6> {
        self.require_active()?;
        let result = allocate_initialized(&mut NativeState { group: self, owner });
        self.finish(result)
    }

    /// Acquire observations bracketed by all-participant idle/currentness checks.
    /// This does not grant completion, another activation, or context teardown.
    pub fn observe_wave_qkv_attention_output_tiles_state_v6(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
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
    pub unsafe fn rearm_wave_qkv_attention_output_tiles_state_v6(
        &mut self,
        state: &mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
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
#[path = "engineering_gfx950_peer_wave_qkv_attention_output_tiles_state_v6_tests.rs"]
mod tests;
