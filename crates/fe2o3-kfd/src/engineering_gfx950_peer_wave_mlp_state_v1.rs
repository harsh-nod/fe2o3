//! Owner-only coherent MLP V1 state retained by the engineering peer group.

use super::*;

const STATE_BYTES: usize = 44;
const STATE_WORDS: usize = 11;
const STATE_KERNARG_OFFSET: u32 = 80;

/// Initialized coherent state for one unchanged eleven-root MLP V1 producer.
///
/// This token cannot be constructed from a PUBLIC VRAM allocation, mapped to a
/// peer, or passed to the generic host read/write methods. Ordinary allocation
/// and observation never reset it; rearm is a separate unsafe engineering API.
/// The group retains it until explicit close or quarantined process teardown.
///
/// ```compile_fail
/// let _ = fe2o3_kfd::Gfx950EngineeringPeerWaveMlpStateV1 { buffer: todo!() };
/// ```
///
/// ```compile_fail
/// fn ordinary_is_not_state(buffer: fe2o3_kfd::Gfx950EngineeringPeerBufferV1) {
///     let _: fe2o3_kfd::Gfx950EngineeringPeerWaveMlpStateV1 = buffer;
/// }
/// ```
#[derive(Debug)]
pub struct Gfx950EngineeringPeerWaveMlpStateV1 {
    buffer: Gfx950EngineeringPeerBufferV1,
}

impl Gfx950EngineeringPeerWaveMlpStateV1 {
    pub const fn owner_rank(&self) -> usize {
        self.buffer.owner
    }

    /// Bind only MLP V1 root ten: byte offset 80, all 11 atomic words.
    /// The unsafe dispatch API still requires the caller to validate the image.
    pub fn pointer_mlp_v1(&self) -> Gfx950EngineeringPeerPointerV1 {
        self.buffer.pointer(
            STATE_KERNARG_OFFSET,
            0,
            STATE_BYTES as u64,
            BufferAccessV1::ReadWrite,
        )
    }
}

fn initial_state() -> [u32; STATE_WORDS] {
    core::array::from_fn(|index| u32::from(index < 2))
}

fn validate_state_record(
    token: &Gfx950EngineeringPeerWaveMlpStateV1,
    record: &BufferRecord,
) -> Result<()> {
    if record.token != token.buffer
        || token.buffer.bytes != STATE_BYTES as u64
        || record.kind != BufferKind::WaveMlpStateV1
        || record.mapping.phase != Phase::PeersMapped
        || !record.mapping.peers.is_empty()
        || record.mapping.mapped != 0
        || record.mapping.unmapped != 0
    {
        return Err("state is not an initialized owner-only MLP V1 allocation".into());
    }
    Ok(())
}

trait StateBackend {
    fn check(&mut self) -> Result<()>;
    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveMlpStateV1>;
    fn initialize(&mut self, state: &Gfx950EngineeringPeerWaveMlpStateV1) -> Result<()>;
    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveMlpStateV1,
    ) -> Result<[u32; STATE_WORDS]>;
}

fn allocate_initialized(
    backend: &mut impl StateBackend,
) -> Result<Gfx950EngineeringPeerWaveMlpStateV1> {
    backend.check()?;
    let state = backend.allocate()?;
    backend.initialize(&state)?;
    if backend.observe(&state)? != initial_state() {
        return Err("MLP V1 state initialization mismatch".into());
    }
    backend.check()?;
    Ok(state)
}

fn observe_idle(
    backend: &mut impl StateBackend,
    state: &Gfx950EngineeringPeerWaveMlpStateV1,
) -> Result<[u32; STATE_WORDS]> {
    backend.check()?;
    let words = backend.observe(state)?;
    backend.check()?;
    Ok(words)
}

trait RearmBackend: StateBackend {
    fn rearm(&mut self, state: &Gfx950EngineeringPeerWaveMlpStateV1) -> Result<()>;
}

fn rearm_terminal(
    backend: &mut impl RearmBackend,
    state: &Gfx950EngineeringPeerWaveMlpStateV1,
    expected: &[u32; STATE_WORDS],
) -> Result<()> {
    if expected[..4] != [1, 0, 31, 31]
        || expected[5] != 0
        || expected[6..] != [64; 5]
        || expected[4] & !0x3ff != 0
        || !(0..5).all(|i| matches!((expected[4] >> (2 * i)) & 3, 1 | 2))
    {
        return Err("MLP rearm requires exact terminal state".into());
    }
    backend.check()?;
    if backend.observe(state)? != *expected {
        return Err("MLP terminal state changed before rearm".into());
    }
    backend.rearm(state)?;
    if backend.observe(state)? != initial_state() {
        return Err("MLP rearm readback mismatch".into());
    }
    backend.check()
}

struct NativeState<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    owner: usize,
}

impl NativeState<'_> {
    fn local_id(&self, state: &Gfx950EngineeringPeerWaveMlpStateV1) -> Result<u64> {
        if self.owner >= self.group.contexts.len() || state.buffer.owner != self.owner {
            return Err("MLP V1 state owner is outside this group".into());
        }
        let record = self.group.validate_token(state.buffer)?;
        validate_state_record(state, record)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get(&record.local_id)
            .ok_or("missing MLP V1 state owner allocation")?;
        if allocation.requested != STATE_BYTES {
            return Err("MLP V1 state allocation extent changed".into());
        }
        Ok(record.local_id)
    }
}

impl StateBackend for NativeState<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveMlpStateV1> {
        if self.owner >= self.group.contexts.len()
            || self.group.buffers.len() >= group_allocation_limit(self.group.contexts.len())?
            || self.group.contexts[self.owner].buffers.len() >= MAX_ALLOCATIONS
        {
            return Err("MLP V1 state allocation bounds".into());
        }
        let id = self.group.next_buffer;
        let next_id = id.checked_add(1).ok_or("peer buffer identity exhausted")?;
        let context = &mut self.group.contexts[self.owner];
        let local_id = context.next_buffer;
        let next_local_id = local_id.checked_add(1).ok_or("buffer ID exhausted")?;
        self.group.next_buffer = next_id;
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
                kind: BufferKind::WaveMlpStateV1,
            },
        );
        let record = self
            .group
            .buffers
            .get_mut(&id)
            .ok_or("missing new state record")?;
        // The empty roster performs owner/currentness checks but no peer map ioctl.
        record.mapping.map(&mut NativeTransaction {
            contexts: &mut self.group.contexts,
            shared_full_currentness: self.group.shared_full_currentness,
            owner: self.owner,
            local_id,
        })?;
        Ok(Gfx950EngineeringPeerWaveMlpStateV1 { buffer: token })
    }

    fn initialize(&mut self, state: &Gfx950EngineeringPeerWaveMlpStateV1) -> Result<()> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing MLP V1 state before initialization")?;
        Backend::initialize_engineering_wave_mlp_task_state_v1(&mut allocation.mapping)
            .map_err(explain)
    }

    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveMlpStateV1,
    ) -> Result<[u32; STATE_WORDS]> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing MLP V1 state before observation")?;
        Backend::observe_engineering_wave_mlp_task_state_v1(&mut allocation.mapping)
            .map_err(explain)
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Allocate and atomically initialize an owner-only coherent MLP V1 state.
    /// Every failure quarantines this group; no reset or generic write is exposed.
    pub fn allocate_wave_mlp_state_v1(
        &mut self,
        owner: usize,
    ) -> Result<Gfx950EngineeringPeerWaveMlpStateV1> {
        self.require_active()?;
        let result = (|| {
            if owner >= self.contexts.len() {
                return Err("MLP V1 state owner is outside this group".into());
            }
            allocate_initialized(&mut NativeState { group: self, owner })
        })();
        self.finish(result)
    }

    /// Acquire-load all state words while every group queue is quiescent.
    /// This observation is not a completion/progress certificate: the caller must
    /// validate every word and retain the exact dispatch/image evidence separately.
    pub fn observe_wave_mlp_state_v1(
        &mut self,
        state: &Gfx950EngineeringPeerWaveMlpStateV1,
    ) -> Result<[u32; STATE_WORDS]> {
        self.require_active()?;
        let result = (|| {
            let mut backend = NativeState {
                group: self,
                owner: state.owner_rank(),
            };
            backend.local_id(state)?;
            observe_idle(&mut backend, state)
        })();
        self.finish(result)
    }

    /// Rearm completed MLP state without changing the pinned device epoch1 ABI.
    /// Any uncertainty quarantines the group; this does not authorize a generation.
    /// # Safety
    /// The caller must own a separately validated monotone-generation ledger,
    /// commit the preceding use and permanently retire every old pointer/command.
    /// No use of this state may overlap this call or escape the same group owner.
    /// Caller identities or an acquired terminal snapshot alone do not establish
    /// those conditions. This API is not selected by the one-shot V1 profile.
    pub unsafe fn rearm_wave_mlp_state_for_reuse_v1(
        &mut self,
        state: &mut Gfx950EngineeringPeerWaveMlpStateV1,
        expected: &[u32; 11],
    ) -> Result<()> {
        self.require_active()?;
        let result = (|| {
            let mut backend = NativeState {
                group: self,
                owner: state.owner_rank(),
            };
            backend.local_id(state)?;
            rearm_terminal(&mut backend, state, expected)
        })();
        self.finish(result)
    }
}

impl RearmBackend for NativeState<'_> {
    fn rearm(&mut self, state: &Gfx950EngineeringPeerWaveMlpStateV1) -> Result<()> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing MLP state during idle rearm")?;
        Backend::rearm_engineering_wave_mlp_task_state_v1(&mut allocation.mapping).map_err(explain)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_mlp_state_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_mlp_rearm_tests.rs"]
mod rearm_tests;
