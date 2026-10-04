//! Exact nine-object state lifetime for the distinct useful multiwave profile.
use super::*;

impl<D: LinuxMemoryDevice> LinuxMemoryBackendFor<D> {
    pub(crate) fn initialize_engineering_multiwave_join_state(
        mapping: &mut LinuxCpuMapping,
    ) -> Result<(), MemorySessionError> {
        let pointer =
            checked_mapping_pointer(mapping, 36, 0, 36, core::mem::align_of::<AtomicU32>())?
                .cast::<AtomicU32>();
        for (index, value) in [1, 3, 0, 0, 0, 0, 0, 0, 0].into_iter().enumerate() {
            // SAFETY: all nine aligned objects are constructed in fresh,
            // exclusively owned storage before any publication.
            unsafe { pointer.add(index).write(AtomicU32::new(value)) };
        }
        Ok(())
    }

    pub(crate) fn observe_engineering_multiwave_join_state(
        mapping: &mut LinuxCpuMapping,
    ) -> Result<[u32; 9], MemorySessionError> {
        let pointer =
            checked_mapping_pointer(mapping, 36, 0, 36, core::mem::align_of::<AtomicU32>())?
                .cast::<AtomicU32>();
        Ok(core::array::from_fn(|index| {
            // SAFETY: callers retain these initialized objects either before
            // publication or after completion and confirmed queue destruction.
            unsafe { (*pointer.add(index)).load(Ordering::Acquire) }
        }))
    }
}

#[cfg(test)]
#[path = "memory_linux_multiwave_join_v1_tests.rs"]
mod tests;
