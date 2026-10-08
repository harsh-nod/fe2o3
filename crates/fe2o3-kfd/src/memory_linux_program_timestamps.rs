//! Separate bounded signal arena; ordered64 keeps its original page limit.

use super::*;

fn program_extent(
    mapping: &mut LinuxCpuMapping,
    requested: usize,
    count: usize,
) -> Result<(), MemorySessionError> {
    let extent = count
        .checked_mul(AMD_SIGNAL_BYTES_V1)
        .and_then(|bytes| bytes.checked_add(4095))
        .map(|bytes| bytes & !4095);
    if !(1..=crate::engineering_wire::MAX_TOKEN_PROGRAM_DISPATCHES_V1).contains(&count)
        || extent != Some(requested)
    {
        return Err(malformed_aql_mapping("native timestamp arena extent"));
    }
    checked_mapping_pointer(mapping, requested, 0, requested, AMD_SIGNAL_BYTES_V1)?;
    Ok(())
}

impl LinuxMemoryBackendFor<crate::CheckedGfx950XnackMinusDevice> {
    /// Caller owns the idle arena and excludes GPU access until publication.
    pub(crate) unsafe fn clear_engineering_program_timestamps(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
        count: usize,
    ) -> Result<(), MemorySessionError> {
        // Validate the full extent before the first write.
        program_extent(mapping, requested, count)?;
        for slot in 0..count {
            let pointer = checked_mapping_pointer(
                mapping,
                requested,
                slot * AMD_SIGNAL_BYTES_V1,
                AMD_SIGNAL_BYTES_V1,
                AMD_SIGNAL_BYTES_V1,
            )?;
            for offset in [START_TICK_OFFSET, END_TICK_OFFSET] {
                // SAFETY: idle exclusive arena, aligned timestamp words only.
                unsafe { pointer.add(offset).cast::<u64>().write_volatile(0) };
            }
        }
        core::sync::atomic::fence(Ordering::Release);
        Ok(())
    }

    /// Caller retains all completed signals and excludes subsequent publication.
    pub(crate) unsafe fn observe_engineering_program_timestamps(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
        count: usize,
        slot: u32,
    ) -> Result<(u64, u64), MemorySessionError> {
        program_extent(mapping, requested, count)?;
        if slot as usize >= count {
            return Err(malformed_aql_mapping("native timestamp slot"));
        }
        let (kind, value) =
            Self::observe_completion_signal_state_acquire(mapping, requested, slot)?;
        if kind != fe2o3_aql::AMD_SIGNAL_KIND_USER_V1 || value != 0 {
            return Err(malformed_aql_mapping("native timestamp signal incomplete"));
        }
        let pointer = checked_mapping_pointer(
            mapping,
            requested,
            slot as usize * AMD_SIGNAL_BYTES_V1,
            AMD_SIGNAL_BYTES_V1,
            AMD_SIGNAL_BYTES_V1,
        )?;
        // SAFETY: acquired completion, retained arena, aligned checked fields.
        let start = unsafe { pointer.add(START_TICK_OFFSET).cast::<u64>().read_volatile() };
        // SAFETY: same completed signal and lifetime as start.
        let end = unsafe { pointer.add(END_TICK_OFFSET).cast::<u64>().read_volatile() };
        Ok((u64::from_le(start), u64::from_le(end)))
    }
}

#[cfg(test)]
#[path = "memory_linux_program_timestamps_tests.rs"]
mod tests;
