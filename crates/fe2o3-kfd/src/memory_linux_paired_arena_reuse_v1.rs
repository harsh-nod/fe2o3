//! Private suffix-only writes; the signal page never becomes an ordinary slice.
use super::*;

const SIGNAL_PAGE: usize = crate::HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize;
const SLOT: usize = crate::engineering_wire::MAX_KERNARG_BYTES_V1 as usize;
const EXTENT: usize = SIGNAL_PAGE + 4 * SLOT;

impl LinuxGfx950MemoryBackend {
    /// # Safety
    /// The private paired coordinator owns the original mapping exclusively.
    /// Both old batches, including the peer barriers and R2, have completed and
    /// hardware read frontiers consumed their packets. No queued or external
    /// reader retains these kernargs. The signal-page atomics remain live.
    /// The source byte slices do not overlap the destination mapping.
    pub(crate) unsafe fn rewrite_paired_kernargs_quiescent_v1(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
        values: [&[u8]; 4],
    ) -> Result<(), MemorySessionError> {
        if requested != EXTENT
            || values.iter().any(|v| v.len() > SLOT)
            || mapping.reservation_phase.load(Ordering::Acquire) != VA_IDENTITY_MAPPED
        {
            return Err(malformed_aql_mapping("paired kernarg reuse shape"));
        }
        // Validate the entire non-atomic suffix before the first write. Do not
        // borrow the preceding page, which contains live AtomicI64 objects.
        let tail = checked_mapping_pointer(mapping, requested, SIGNAL_PAGE, 4 * SLOT, 1)?;
        for (index, value) in values.into_iter().enumerate() {
            // SAFETY: checked non-overlapping suffix ranges; caller proves all
            // old GPU readers retired. Exact immutable profiles bound reads to
            // these initialized bytes; unused slot padding is never exposed.
            unsafe {
                core::ptr::copy_nonoverlapping(value.as_ptr(), tail.add(index * SLOT), value.len())
            };
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "memory_linux_paired_arena_reuse_v1_tests.rs"]
mod tests;
