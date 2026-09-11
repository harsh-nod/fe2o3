//! One synchronous initialization sequence, with concrete private allocation tokens.

use super::*;

pub(super) type CpuAllocation = SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>;
pub(super) type MappedAllocation =
    SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>;

pub(super) trait CoherentInitializationV1 {
    fn allocate(&mut self, length: usize) -> Result<CpuAllocation, MemorySessionError>;
    fn copy(
        &mut self,
        allocation: &mut CpuAllocation,
        source: &[u8],
    ) -> Result<(), MemorySessionError>;
    fn map(&mut self, allocation: CpuAllocation) -> Result<MappedAllocation, MemorySessionError>;
}

pub(super) fn initialize_v1(
    memory: &mut impl CoherentInitializationV1,
    source: &[u8],
) -> Result<Gfx942InitializedHostVisibleMemoryV1, MemorySessionError> {
    if source.is_empty() {
        return Err(MemorySessionError::InvalidRequestedSize);
    }
    let mut allocation = memory.allocate(source.len())?;
    memory.copy(&mut allocation, source)?;
    let token = memory.map(allocation)?;
    Ok(Gfx942InitializedHostVisibleMemoryV1 { token })
}

impl CoherentInitializationV1 for SharedGttMemorySessionV1 {
    fn allocate(&mut self, length: usize) -> Result<CpuAllocation, MemorySessionError> {
        self.allocate_host_visible_coherent(length)
    }

    fn copy(
        &mut self,
        allocation: &mut CpuAllocation,
        source: &[u8],
    ) -> Result<(), MemorySessionError> {
        self.with_bytes_mut(allocation, |destination| {
            destination.copy_from_slice(source)
        })
    }

    fn map(&mut self, allocation: CpuAllocation) -> Result<MappedAllocation, MemorySessionError> {
        self.map_to_gpu(allocation)
    }
}
