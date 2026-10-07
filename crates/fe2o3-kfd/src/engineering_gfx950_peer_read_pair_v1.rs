//! A closed TP2 copy transaction; no reusable currentness certificate escapes.
use super::*;

trait ReadPairBackend {
    fn validate(&mut self) -> Result<()>;
    fn fence(&mut self) -> Result<()>;
    fn copy(&mut self, rank: usize) -> Result<Vec<u8>>;
    fn quarantine(&mut self);
}

struct Custody<'a, B: ReadPairBackend> {
    backend: &'a mut B,
    committed: bool,
}
impl<B: ReadPairBackend> Drop for Custody<'_, B> {
    fn drop(&mut self) {
        if !self.committed {
            self.backend.quarantine();
        }
    }
}

fn read_pair(backend: &mut impl ReadPairBackend) -> Result<[Vec<u8>; 2]> {
    let mut custody = Custody {
        backend,
        committed: false,
    };
    custody.backend.validate()?;
    custody.backend.fence()?;
    let first = custody.backend.copy(0)?;
    let second = custody.backend.copy(1)?;
    custody.backend.fence()?;
    custody.committed = true;
    Ok([first, second])
}

fn validate_headers(
    incarnation: u64,
    participants: usize,
    buffers: [Gfx950EngineeringPeerBufferV1; 2],
    offset: u64,
    bytes: u32,
) -> Result<()> {
    if incarnation == 0
        || participants != 2
        || bytes == 0
        || u64::from(bytes) * 2 > u64::from(MAX_TRANSFER_BYTES_V1)
    {
        return Err("peer two-rank read participants/aggregate transfer bound".into());
    }
    for (rank, buffer) in buffers.iter().enumerate() {
        if buffer.group != incarnation || buffer.owner != rank || buffer.id == 0 {
            return Err("peer two-rank read rank-specific identity".into());
        }
        checked_range(buffer.bytes, offset, u64::from(bytes)).map_err(str::to_owned)?;
    }
    if buffers[0].id == buffers[1].id {
        return Err("peer two-rank read requires distinct allocations".into());
    }
    Ok(())
}

struct NativeReadPair<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    buffers: [Gfx950EngineeringPeerBufferV1; 2],
    offset: u64,
    bytes: u32,
    local: [u64; 2],
}
impl ReadPairBackend for NativeReadPair<'_> {
    fn validate(&mut self) -> Result<()> {
        self.group.require_active()?;
        validate_headers(
            self.group.incarnation,
            self.group.contexts.len(),
            self.buffers,
            self.offset,
            self.bytes,
        )?;
        // Authenticate both complete tokens and local extents before either copy.
        for rank in 0..2 {
            let record = self.group.validate_token(self.buffers[rank])?;
            require_public_vram(record)?;
            let allocation = self.group.contexts[rank]
                .buffers
                .get(&record.local_id)
                .ok_or("unknown pair-read local buffer")?;
            if allocation.requested as u64 != self.buffers[rank].bytes {
                return Err("peer two-rank read local extent differs".into());
            }
            checked_range(
                allocation.requested as u64,
                self.offset,
                u64::from(self.bytes),
            )
            .map_err(str::to_owned)?;
            self.local[rank] = record.local_id;
        }
        Ok(())
    }
    fn fence(&mut self) -> Result<()> {
        // Each call performs a fresh full group observation, then validates both
        // physical queues and exceptions. This does not configure a global policy.
        run_context_fence(&mut NativeContextFence(&mut self.group.contexts), true)
    }
    fn copy(&mut self, rank: usize) -> Result<Vec<u8>> {
        let context = &mut self.group.contexts[rank];
        let started = context.profile_started();
        // Only this closed transaction can call here, immediately after its
        // fresh group fence. No callback, dispatch, or mutable buffer escapes.
        context.check_idle_after_currentness()?;
        let allocation = context
            .buffers
            .get(&self.local[rank])
            .ok_or("unknown pair-read local buffer")?;
        let range = checked_range(
            allocation.requested as u64,
            self.offset,
            u64::from(self.bytes),
        )
        .map_err(str::to_owned)?;
        let result = Backend::with_bytes(&allocation.mapping, allocation.requested, |mapped| {
            mapped[range].to_vec()
        });
        if started.is_some() {
            add_counter(&mut context.counters.reads, 1)?;
            add_counter(&mut context.counters.read_bytes, u64::from(self.bytes))?;
            record_elapsed(&mut context.counters.read_ns, started)?;
        }
        Ok(result)
    }
    fn quarantine(&mut self) {
        self.group.poisoned = true;
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Reads one PUBLIC-VRAM allocation from each of exactly two ranks.
    ///
    /// Both rank-specific tokens and ranges are checked before either copy.
    /// Two fresh full-group fences bound the operation; no data is returned if
    /// either queue/currentness/copy check fails. The combined transfer must fit
    /// the existing transfer limit. This is an engineering observation only,
    /// not a reusable currentness proof or an atomic cross-rank device snapshot.
    /// The ordinary one-buffer `read` method and global policies are unchanged.
    pub fn read_pair_v1(
        &mut self,
        buffers: [Gfx950EngineeringPeerBufferV1; 2],
        offset: u64,
        bytes: u32,
    ) -> Result<[Vec<u8>; 2]> {
        self.require_active()?;
        read_pair(&mut NativeReadPair {
            group: self,
            buffers,
            offset,
            bytes,
            local: [0; 2],
        })
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_read_pair_v1_tests.rs"]
mod tests;
