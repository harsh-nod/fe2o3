//! Owner-only coherent V5 state retained by the engineering peer group.

use super::*;

const STATE_BYTES: usize = 88;
const STATE_WORDS: usize = 22;
const STATE_KERNARG_OFFSET: u32 = 112;

/// Initialized coherent state for one unchanged fifteen-root Output V5 producer.
///
/// This token cannot be constructed from a PUBLIC VRAM allocation, mapped to a
/// peer, or passed to the generic host read/write methods. Ordinary allocation
/// and observation never reset it; rearm is a separate unsafe engineering API.
/// The group retains it until explicit close or quarantined process teardown.
///
/// ```compile_fail
/// let _ = fe2o3_kfd::Gfx950EngineeringPeerWaveOutputStateV5 { buffer: todo!() };
/// ```
///
/// ```compile_fail
/// fn ordinary_is_not_state(buffer: fe2o3_kfd::Gfx950EngineeringPeerBufferV1) {
///     let _: fe2o3_kfd::Gfx950EngineeringPeerWaveOutputStateV5 = buffer;
/// }
/// ```
#[derive(Debug)]
pub struct Gfx950EngineeringPeerWaveOutputStateV5 {
    buffer: Gfx950EngineeringPeerBufferV1,
}

impl Gfx950EngineeringPeerWaveOutputStateV5 {
    pub const fn owner_rank(&self) -> usize {
        self.buffer.owner
    }

    /// Bind only V5 root fourteen: byte offset 112, all 22 atomic words.
    /// The unsafe dispatch API still requires the caller to validate the image.
    pub fn pointer_v5(&self) -> Gfx950EngineeringPeerPointerV1 {
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
    token: &Gfx950EngineeringPeerWaveOutputStateV5,
    record: &BufferRecord,
) -> Result<()> {
    if record.token != token.buffer
        || token.buffer.bytes != STATE_BYTES as u64
        || record.kind != BufferKind::WaveOutputStateV5
        || record.mapping.phase != Phase::PeersMapped
        || !record.mapping.peers.is_empty()
        || record.mapping.mapped != 0
        || record.mapping.unmapped != 0
    {
        return Err("state is not an initialized owner-only Output V5 allocation".into());
    }
    Ok(())
}

trait StateBackend {
    fn check(&mut self) -> Result<()>;
    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveOutputStateV5>;
    fn initialize(&mut self, state: &Gfx950EngineeringPeerWaveOutputStateV5) -> Result<()>;
    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveOutputStateV5,
    ) -> Result<[u32; STATE_WORDS]>;
}

fn allocate_initialized(
    backend: &mut impl StateBackend,
) -> Result<Gfx950EngineeringPeerWaveOutputStateV5> {
    backend.check()?;
    let state = backend.allocate()?;
    backend.initialize(&state)?;
    if backend.observe(&state)? != initial_state() {
        return Err("Output V5 state initialization mismatch".into());
    }
    backend.check()?;
    Ok(state)
}

fn observe_idle(
    backend: &mut impl StateBackend,
    state: &Gfx950EngineeringPeerWaveOutputStateV5,
) -> Result<[u32; STATE_WORDS]> {
    backend.check()?;
    let words = backend.observe(state)?;
    backend.check()?;
    Ok(words)
}

trait RearmBackend: StateBackend {
    fn rearm(&mut self, state: &Gfx950EngineeringPeerWaveOutputStateV5) -> Result<()>;
}

fn rearm_terminal(
    backend: &mut impl RearmBackend,
    state: &Gfx950EngineeringPeerWaveOutputStateV5,
    expected: &[u32; STATE_WORDS],
) -> Result<()> {
    if expected[..4] != [1, 0, 0xffff, 0xffff]
        || expected[5] != 0
        || expected[6..] != [64; 16]
        || !(0..16).all(|i| matches!((expected[4] >> (2 * i)) & 3, 1 | 2))
    {
        return Err("Output V5 rearm requires exact terminal state".into());
    }
    backend.check()?;
    if backend.observe(state)? != *expected {
        return Err("Output terminal state changed before rearm".into());
    }
    backend.rearm(state)?;
    if backend.observe(state)? != initial_state() {
        return Err("Output rearm readback mismatch".into());
    }
    backend.check()
}

struct NativeState<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    owner: usize,
}

impl NativeState<'_> {
    fn local_id(&self, state: &Gfx950EngineeringPeerWaveOutputStateV5) -> Result<u64> {
        if self.owner >= self.group.contexts.len() || state.buffer.owner != self.owner {
            return Err("Output V5 state owner is outside this group".into());
        }
        let record = self.group.validate_token(state.buffer)?;
        validate_state_record(state, record)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get(&record.local_id)
            .ok_or("missing Output V5 state owner allocation")?;
        if allocation.requested != STATE_BYTES {
            return Err("Output V5 state allocation extent changed".into());
        }
        Ok(record.local_id)
    }
}

impl StateBackend for NativeState<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveOutputStateV5> {
        if self.owner >= self.group.contexts.len()
            || self.group.buffers.len() >= group_allocation_limit(self.group.contexts.len())?
            || self.group.contexts[self.owner].buffers.len() >= MAX_ALLOCATIONS
        {
            return Err("Output V5 state allocation bounds".into());
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
                kind: BufferKind::WaveOutputStateV5,
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
        Ok(Gfx950EngineeringPeerWaveOutputStateV5 { buffer: token })
    }

    fn initialize(&mut self, state: &Gfx950EngineeringPeerWaveOutputStateV5) -> Result<()> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing Output V5 state before initialization")?;
        Backend::initialize_engineering_wave_qkv_attention_output_task_state_v5(
            &mut allocation.mapping,
        )
        .map_err(explain)
    }

    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveOutputStateV5,
    ) -> Result<[u32; STATE_WORDS]> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing Output V5 state before observation")?;
        Backend::observe_engineering_wave_qkv_attention_output_task_state_v5(
            &mut allocation.mapping,
        )
        .map_err(explain)
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Allocate and atomically initialize an owner-only coherent V5 state.
    /// Every failure quarantines this group; no reset or generic write is exposed.
    pub fn allocate_wave_output_state_v5(
        &mut self,
        owner: usize,
    ) -> Result<Gfx950EngineeringPeerWaveOutputStateV5> {
        self.require_active()?;
        let result = (|| {
            if owner >= self.contexts.len() {
                return Err("Output V5 state owner is outside this group".into());
            }
            allocate_initialized(&mut NativeState { group: self, owner })
        })();
        self.finish(result)
    }

    /// Acquire-load all state words while every group queue is quiescent.
    /// This observation is not a completion/progress certificate: the caller must
    /// validate every word and retain the exact dispatch/image evidence separately.
    pub fn observe_wave_output_state_v5(
        &mut self,
        state: &Gfx950EngineeringPeerWaveOutputStateV5,
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

    /// Rearm completed Output state without changing the device epoch1 ABI.
    /// Any uncertainty quarantines the group; no generation authority is minted.
    /// # Safety
    /// The caller must own a separately validated monotone-generation ledger,
    /// commit the preceding use and permanently retire every old pointer/command.
    /// No use of this state may overlap this call or escape the same group owner.
    /// A caller-supplied identity or snapshot is insufficient. The ordinary
    /// one-shot V5 profile does not select this API.
    pub unsafe fn rearm_wave_output_state_for_reuse_v1(
        &mut self,
        state: &mut Gfx950EngineeringPeerWaveOutputStateV5,
        expected: &[u32; 22],
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
    fn rearm(&mut self, state: &Gfx950EngineeringPeerWaveOutputStateV5) -> Result<()> {
        let local_id = self.local_id(state)?;
        let allocation = self.group.contexts[self.owner]
            .buffers
            .get_mut(&local_id)
            .ok_or("missing Output state during idle rearm")?;
        Backend::rearm_engineering_wave_output_task_state_v1(&mut allocation.mapping)
            .map_err(explain)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_output_state_v5_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_output_rearm_tests.rs"]
mod rearm_tests;
