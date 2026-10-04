//! Raw ROCr 6.4.3 queue/signal ABI access for opt-in engineering diagnostics.

use super::*;

// ROCr rocm-6.4.3: runtime/hsa-runtime/inc/amd_hsa_queue.h and amd_hsa_signal.h.
const QUEUE_PROPERTIES_OFFSET: usize = 0xb4;
const ENABLE_PROFILING: u32 = 1 << 3;
const START_TICK_OFFSET: usize = 32;
const END_TICK_OFFSET: usize = 40;

impl LinuxMemoryBackendFor<crate::CheckedGfx950XnackMinusDevice> {
    /// Caller must own an idle engineering queue with no outstanding packets.
    pub(crate) unsafe fn enable_engineering_dispatch_timestamps(
        mapping: &mut LinuxCpuMapping,
    ) -> Result<u32, MemorySessionError> {
        let pointer = checked_mapping_pointer(mapping, 4096, QUEUE_PROPERTIES_OFFSET, 4, 4)?;
        // SAFETY: this is the checked CPU-owned property word, not a counter.
        let old = u32::from_le(unsafe { pointer.cast::<u32>().read_volatile() });
        if old != 0 {
            return Err(malformed_aql_mapping(
                "engineering timestamp initial properties",
            ));
        }
        // SAFETY: exclusive idle-queue contract; preserve every unrelated bit.
        unsafe {
            pointer
                .cast::<u32>()
                .write_volatile((old | ENABLE_PROFILING).to_le())
        };
        core::sync::atomic::fence(Ordering::Release);
        Ok(old)
    }

    /// Caller must have observed all packets complete and revalidated idle.
    pub(crate) unsafe fn restore_engineering_dispatch_timestamps(
        mapping: &mut LinuxCpuMapping,
        saved: u32,
    ) -> Result<(), MemorySessionError> {
        let pointer = checked_mapping_pointer(mapping, 4096, QUEUE_PROPERTIES_OFFSET, 4, 4)?;
        // SAFETY: same exclusive idle-queue contract as enable.
        let current = u32::from_le(unsafe { pointer.cast::<u32>().read_volatile() });
        if saved != 0 || current != (saved | ENABLE_PROFILING) {
            return Err(malformed_aql_mapping(
                "engineering timestamp properties changed",
            ));
        }
        // SAFETY: restore the exact prior word only after successful execution.
        unsafe { pointer.cast::<u32>().write_volatile(saved.to_le()) };
        core::sync::atomic::fence(Ordering::Release);
        Ok(())
    }

    /// Caller must own idle signal storage with no outstanding GPU references.
    pub(crate) unsafe fn clear_engineering_dispatch_timestamps(
        mapping: &mut LinuxCpuMapping,
        count: usize,
    ) -> Result<(), MemorySessionError> {
        if !(1..=crate::engineering_wire::MAX_ORDERED_BATCH64_DISPATCHES_V1).contains(&count) {
            return Err(malformed_aql_mapping("engineering timestamp slot count"));
        }
        for slot in 0..count {
            let signal = timestamp_slot(mapping, slot as u32)?;
            for offset in [START_TICK_OFFSET, END_TICK_OFFSET] {
                // SAFETY: full 64-byte aligned slot checked; these u64 fields
                // are not signal counters, and the caller proved idle reuse.
                unsafe { signal.add(offset).cast::<u64>().write_volatile(0) };
            }
        }
        core::sync::atomic::fence(Ordering::Release);
        Ok(())
    }

    /// Caller retains the completed signal without any subsequent publication.
    pub(crate) unsafe fn observe_engineering_dispatch_timestamps(
        mapping: &mut LinuxCpuMapping,
        slot: u32,
    ) -> Result<(u64, u64), MemorySessionError> {
        let (kind, value) = Self::observe_completion_signal_state_acquire(mapping, 4096, slot)?;
        if kind != fe2o3_aql::AMD_SIGNAL_KIND_USER_V1 || value != 0 {
            return Err(malformed_aql_mapping(
                "engineering timestamp signal incomplete",
            ));
        }
        let signal = timestamp_slot(mapping, slot)?;
        // SAFETY: completion was acquired and caller forbids signal reuse;
        // both firmware-written u64 fields fit inside the checked aligned slot.
        let start = unsafe { signal.add(START_TICK_OFFSET).cast::<u64>().read_volatile() };
        // SAFETY: same completed, retained signal extent as start.
        let end = unsafe { signal.add(END_TICK_OFFSET).cast::<u64>().read_volatile() };
        Ok((u64::from_le(start), u64::from_le(end)))
    }
}

fn timestamp_slot(mapping: &mut LinuxCpuMapping, slot: u32) -> Result<*mut u8, MemorySessionError> {
    if slot as usize >= crate::engineering_wire::MAX_ORDERED_BATCH64_DISPATCHES_V1 {
        return Err(malformed_aql_mapping("engineering timestamp slot index"));
    }
    checked_mapping_pointer(
        mapping,
        4096,
        slot as usize * AMD_SIGNAL_BYTES_V1,
        AMD_SIGNAL_BYTES_V1,
        AMD_SIGNAL_BYTES_V1,
    )
}

#[cfg(test)]
#[path = "memory_linux_dispatch_timestamps_tests.rs"]
mod tests;
