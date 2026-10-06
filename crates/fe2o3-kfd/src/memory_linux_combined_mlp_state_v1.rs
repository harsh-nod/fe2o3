//! Private genuine-atomic storage for the additive combined MLP owner.
use super::*;

const PREFIX_WORDS: usize = 548;
const WORDS: usize = PREFIX_WORDS + 4;
const BYTES: usize = WORDS * core::mem::size_of::<AtomicU32>();

fn combined_pointer(
    mapping: &mut LinuxCpuMapping,
    requested: usize,
) -> Result<*mut AtomicU32, MemorySessionError> {
    if requested != BYTES {
        return Err(malformed_aql_mapping("combined MLP exact atomic extent"));
    }
    Ok(checked_mapping_pointer(
        mapping,
        requested,
        0,
        BYTES,
        core::mem::align_of::<AtomicU32>(),
    )?
    .cast())
}

#[cfg(test)]
#[path = "memory_linux_combined_mlp_state_v1_tests.rs"]
mod tests;

impl LinuxGfx950MemoryBackend {
    /// Test-only snapshot seeding, not an owner transition or quiescence proof.
    ///
    /// # Safety
    /// All 552 atomic lifetimes already exist. The diagnostic caller owns the
    /// entire group before its first dispatch or after every synchronous call
    /// has completed successfully. No GPU or host observer may overlap stores.
    #[cfg(all(test, feature = "engineering-gfx950"))]
    pub(crate) unsafe fn seed_combined_snapshot_quiescent_v1(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
        prefix: &[u32; PREFIX_WORDS],
        guard: [u32; 4],
    ) -> Result<(), MemorySessionError> {
        let pointer = combined_pointer(mapping, requested)?;
        // Preserve every atomic object; never create a byte slice over them.
        unsafe { &*pointer.add(PREFIX_WORDS + 2) }.store(0, Ordering::Release);
        for (index, value) in prefix.iter().copied().enumerate() {
            unsafe { &*pointer.add(index) }.store(value, Ordering::Relaxed);
        }
        for index in [0, 1, 3] {
            unsafe { &*pointer.add(PREFIX_WORDS + index) }.store(guard[index], Ordering::Relaxed);
        }
        unsafe { &*pointer.add(PREFIX_WORDS + 2) }.store(guard[2], Ordering::Release);
        Ok(())
    }

    /// # Safety
    /// Fresh private storage, no prior atomic lifetimes and no published GPU
    /// address. The retained owner must prohibit all ordinary byte access.
    #[cfg(feature = "engineering-gfx950")]
    pub(crate) unsafe fn initialize_combined_mlp_state_v1(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
        initial: &[u32; PREFIX_WORDS],
        generation: u64,
    ) -> Result<(), MemorySessionError> {
        if generation == 0 {
            return Err(malformed_aql_mapping("combined MLP generation zero"));
        }
        let pointer = combined_pointer(mapping, requested)?;
        let guard = [generation as u32, (generation >> 32) as u32, 0, 0];
        for (index, value) in initial.iter().chain(guard.iter()).copied().enumerate() {
            // SAFETY: caller supplies fresh, aligned, exclusive storage.
            unsafe { pointer.add(index).write(AtomicU32::new(value)) };
        }
        Ok(())
    }

    /// # Safety
    /// All 552 atomic objects are initialized and retained. Authoritative use
    /// additionally requires the coordinator's complete group-quiescence proof.
    #[cfg(feature = "engineering-gfx950")]
    pub(crate) unsafe fn observe_combined_mlp_state_v1(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
    ) -> Result<([u32; PREFIX_WORDS], [u32; 4]), MemorySessionError> {
        let pointer = combined_pointer(mapping, requested)?;
        // SAFETY: initialized atomics live until queue-first group Close.
        let verdict = unsafe { &*pointer.add(PREFIX_WORDS + 2) }.load(Ordering::Acquire);
        let guard = [
            unsafe { &*pointer.add(PREFIX_WORDS) }.load(Ordering::Acquire),
            unsafe { &*pointer.add(PREFIX_WORDS + 1) }.load(Ordering::Acquire),
            verdict,
            unsafe { &*pointer.add(PREFIX_WORDS + 3) }.load(Ordering::Acquire),
        ];
        let prefix =
            core::array::from_fn(|index| unsafe { &*pointer.add(index) }.load(Ordering::Acquire));
        Ok((prefix, guard))
    }

    /// # Safety
    /// Both ranks are quiescent after the previous generation's terminal
    /// validation. No old R2 reader, validator, MLP writer or queued reference
    /// remains; the coordinator entered Busy before this call and has not
    /// published the next generation. Atomic lifetimes already exist.
    #[cfg(feature = "engineering-gfx950")]
    pub(crate) unsafe fn rearm_combined_mlp_state_v1(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
        initial: &[u32; PREFIX_WORDS],
        generation: u64,
    ) -> Result<(), MemorySessionError> {
        if generation == 0 {
            return Err(malformed_aql_mapping("combined MLP generation zero"));
        }
        let pointer = combined_pointer(mapping, requested)?;
        // SAFETY: caller established quiescence; stores preserve atomic objects.
        unsafe { &*pointer.add(PREFIX_WORDS + 2) }.store(0, Ordering::Release);
        for (index, value) in initial.iter().copied().enumerate() {
            unsafe { &*pointer.add(index) }.store(value, Ordering::Release);
        }
        unsafe { &*pointer.add(PREFIX_WORDS) }.store(generation as u32, Ordering::Relaxed);
        unsafe { &*pointer.add(PREFIX_WORDS + 1) }
            .store((generation >> 32) as u32, Ordering::Relaxed);
        unsafe { &*pointer.add(PREFIX_WORDS + 3) }.store(0, Ordering::Relaxed);
        unsafe { &*pointer.add(PREFIX_WORDS + 2) }.store(0, Ordering::Release);
        Ok(())
    }
}
