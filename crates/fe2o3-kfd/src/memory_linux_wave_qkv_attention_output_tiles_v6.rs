//! Private atomic construction/acquire and quiescent stores for V6 owners.
use super::*;

const WORDS: usize = 284;
const BYTES: usize = WORDS * core::mem::size_of::<AtomicU32>();

fn state_pointer(
    mapping: &mut LinuxCpuMapping,
    requested: usize,
) -> Result<*mut AtomicU32, MemorySessionError> {
    // The page backing is also large enough for older states; its size is not
    // a source-profile identity. The retained allocation's exact request is owed.
    if requested != BYTES {
        return Err(malformed_aql_mapping("prefix tile V6 atomic state extent"));
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

impl LinuxGfx950MemoryBackend {
    /// Caller owns initialized V6 atomics, completed/retired prior commands,
    /// exact terminal readback and exclusive quiescence of the whole group.
    pub(crate) fn rearm_engineering_wave_qkv_attention_output_tile_state_v6(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
    ) -> Result<(), MemorySessionError> {
        let pointer = state_pointer(mapping, requested)?;
        for index in 0..WORDS {
            // SAFETY: the retained typed owner supplies initialized atomics;
            // stores preserve their lifetime instead of constructing them again.
            unsafe { &*pointer.add(index) }
                .store(u32::from(matches!(index, 0 | 2)), Ordering::Release);
        }
        Ok(())
    }

    /// Caller retains a fresh private V6 allocation and has not published it.
    pub(crate) fn initialize_engineering_wave_qkv_attention_output_tile_state_v6(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
    ) -> Result<(), MemorySessionError> {
        let pointer = state_pointer(mapping, requested)?;
        for index in 0..WORDS {
            let value = u32::from(matches!(index, 0 | 2));
            // SAFETY: exact live aligned exclusive storage; the one-shot owner
            // prohibits late/double construction and all byte access to state.
            unsafe { pointer.add(index).write(AtomicU32::new(value)) };
        }
        Ok(())
    }

    /// Caller owes initialized atomics and queue completion before final use.
    pub(crate) fn observe_engineering_wave_qkv_attention_output_tile_state_v6(
        mapping: &mut LinuxCpuMapping,
        requested: usize,
    ) -> Result<[u32; WORDS], MemorySessionError> {
        let pointer = state_pointer(mapping, requested)?;
        Ok(core::array::from_fn(|index| {
            // SAFETY: only the owning V6 transaction supplies initialized state.
            unsafe { &*pointer.add(index) }.load(Ordering::Acquire)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backing() -> Box<[AtomicU32; 1024]> {
        Box::new(core::array::from_fn(|_| AtomicU32::new(0x5a5a_1234)))
    }
    fn mapping(words: &mut [AtomicU32; 1024]) -> LinuxCpuMapping {
        LinuxCpuMapping {
            address: NonNull::from(words).cast(),
            bytes: 4096,
            active: true,
            accessible: true,
            reservation_phase: Arc::new(AtomicU8::new(VA_IDENTITY_MAPPED)),
        }
    }

    #[test]
    fn rearm_stores_all_284_initialized_atomics_and_preserves_page_tail() {
        let mut words = backing();
        let mut map = mapping(&mut words);
        LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_output_tile_state_v6(
            &mut map, BYTES,
        )
        .unwrap();
        for (index, word) in words[..WORDS].iter().enumerate() {
            word.store(index as u32 + 1, Ordering::Release);
        }
        LinuxGfx950MemoryBackend::rearm_engineering_wave_qkv_attention_output_tile_state_v6(
            &mut map, BYTES,
        )
        .unwrap();
        assert_eq!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_tile_state_v6(
                &mut map, BYTES
            )
            .unwrap(),
            core::array::from_fn(|index| u32::from(matches!(index, 0 | 2)))
        );
        assert!(
            words[WORDS..]
                .iter()
                .all(|word| word.load(Ordering::Acquire) == 0x5a5a_1234)
        );
    }

    #[test]
    fn rearm_refuses_wrong_request_and_mapping_without_any_atomic_store() {
        for requested in [0, 44, 88, 2192, BYTES - 1, BYTES + 1, 4096, usize::MAX] {
            let mut words = backing();
            let mut map = mapping(&mut words);
            assert!(
                LinuxGfx950MemoryBackend::rearm_engineering_wave_qkv_attention_output_tile_state_v6(
                    &mut map, requested
                )
                .is_err()
            );
            assert!(
                words
                    .iter()
                    .all(|word| word.load(Ordering::Acquire) == 0x5a5a_1234)
            );
        }
        for change in 0..4 {
            let mut words = backing();
            let mut map = mapping(&mut words);
            match change {
                0 => map.bytes = BYTES - 1,
                1 => map.active = false,
                2 => map.accessible = false,
                _ => {
                    map.address =
                        NonNull::new(map.address.as_ptr().cast::<u8>().wrapping_add(1).cast())
                            .unwrap()
                }
            }
            assert!(
                LinuxGfx950MemoryBackend::rearm_engineering_wave_qkv_attention_output_tile_state_v6(&mut map, BYTES)
                    .is_err()
            );
            assert!(
                words
                    .iter()
                    .all(|word| word.load(Ordering::Acquire) == 0x5a5a_1234)
            );
        }
    }

    #[test]
    fn constructs_all_284_real_atomics_without_touching_page_tail() {
        let mut words = backing();
        let mut map = mapping(&mut words);
        LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_output_tile_state_v6(
            &mut map, BYTES,
        )
        .unwrap();
        let observed =
            LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_tile_state_v6(
                &mut map, BYTES,
            )
            .unwrap();
        for (index, value) in observed.into_iter().enumerate() {
            assert_eq!(value, u32::from(matches!(index, 0 | 2)));
        }
        assert!(
            words[WORDS..]
                .iter()
                .all(|word| word.load(Ordering::Relaxed) == 0x5a5a_1234)
        );
        // Real atomic stores model terminal fields without byte reinitialization.
        for (index, word) in words[..WORDS].iter().enumerate() {
            word.store(index as u32, Ordering::Release);
        }
        assert_eq!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_tile_state_v6(
                &mut map, BYTES
            )
            .unwrap(),
            core::array::from_fn(|index| index as u32)
        );
    }

    #[test]
    fn refuses_old_same_page_state_short_dead_inaccessible_and_unaligned_mappings() {
        for requested in [0, 44, 88, 2192, BYTES - 1, BYTES + 1, 4096, usize::MAX] {
            let mut words = backing();
            let mut map = mapping(&mut words);
            assert!(
                LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_output_tile_state_v6(
                    &mut map, requested
                )
                .is_err()
            );
            assert!(
                LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_tile_state_v6(
                    &mut map, requested
                )
                .is_err()
            );
            assert!(
                words
                    .iter()
                    .all(|word| word.load(Ordering::Relaxed) == 0x5a5a_1234)
            );
        }
        for change in 0..4 {
            let mut words = backing();
            let mut map = mapping(&mut words);
            match change {
                0 => map.bytes = BYTES - 1,
                1 => map.active = false,
                2 => map.accessible = false,
                _ => {
                    map.address =
                        NonNull::new(map.address.as_ptr().cast::<u8>().wrapping_add(1).cast())
                            .unwrap()
                }
            }
            assert!(
                LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_output_tile_state_v6(
                    &mut map, BYTES
                )
                .is_err()
            );
            assert!(
                LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_tile_state_v6(
                    &mut map, BYTES
                )
                .is_err()
            );
            assert!(
                words
                    .iter()
                    .all(|word| word.load(Ordering::Relaxed) == 0x5a5a_1234)
            );
        }
    }
}
