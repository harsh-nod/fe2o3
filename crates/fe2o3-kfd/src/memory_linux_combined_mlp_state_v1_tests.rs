use super::*;

#[repr(align(4096))]
struct Storage([u8; 4096]);

fn initial() -> [u32; 548] {
    core::array::from_fn(|index| u32::from(matches!(index, 0 | 2)))
}

fn with_fresh(test: impl FnOnce(&mut LinuxCpuMapping, *mut u8)) {
    let mut storage = Box::new(Storage([0xa5; 4096]));
    let address = NonNull::from(&mut *storage).cast();
    let mut mapping = LinuxCpuMapping {
        address,
        bytes: 4096,
        active: true,
        accessible: true,
        reservation_phase: Arc::new(AtomicU8::new(VA_IDENTITY_MAPPED)),
    };
    test(&mut mapping, address.as_ptr().cast());
}

fn with_initialized(generation: u64, test: impl FnOnce(&mut LinuxCpuMapping, *mut u8)) {
    with_fresh(|mapping, raw| {
        // SAFETY: fresh aligned fixture; no device/thread or prior atomic object.
        unsafe {
            LinuxGfx950MemoryBackend::initialize_combined_mlp_state_v1(
                mapping,
                2208,
                &initial(),
                generation,
            )
        }
        .unwrap();
        test(mapping, raw);
    });
}

fn observe(mapping: &mut LinuxCpuMapping) -> ([u32; 548], [u32; 4]) {
    // SAFETY: this helper is used only after fixture initialization, without GPU.
    unsafe { LinuxGfx950MemoryBackend::observe_combined_mlp_state_v1(mapping, 2208) }.unwrap()
}

fn tail_is_untouched(raw: *mut u8) -> bool {
    // SAFETY: only the non-atomic tail is read as bytes; no whole-object slice.
    (2208..4096).all(|index| unsafe { raw.add(index).read() } == 0xa5)
}

#[test]
fn combined_memory_diagnostic_seed_preserves_objects_and_refuses_wrong_extent() {
    with_initialized(1, |mapping, raw| {
        for verdict in [0, 1, 2, u32::MAX] {
            let prefix = core::array::from_fn(|i| (i as u32).wrapping_mul(17));
            let guard = [0x89ab_cdef, 0x0123_4567, verdict, 1];
            // SAFETY: initialized isolated fixture, no queues or other readers.
            unsafe {
                LinuxGfx950MemoryBackend::seed_combined_snapshot_quiescent_v1(
                    mapping, 2208, &prefix, guard,
                )
            }
            .unwrap();
            assert_eq!(observe(mapping), (prefix, guard));
            assert!(tail_is_untouched(raw));
            for requested in [0, 2192, 2207, 2209, 4096, usize::MAX] {
                // SAFETY: malformed extent must refuse before any store.
                assert!(
                    unsafe {
                        LinuxGfx950MemoryBackend::seed_combined_snapshot_quiescent_v1(
                            mapping,
                            requested,
                            &initial(),
                            [0; 4],
                        )
                    }
                    .is_err()
                );
                assert_eq!(observe(mapping), (prefix, guard));
                assert!(tail_is_untouched(raw));
            }
        }
    });
}

#[test]
fn combined_memory_constructs_exact_552_genuine_atomics_and_preserves_tail() {
    for generation in [1, (1 << 32) + 9, u64::MAX] {
        with_initialized(generation, |mapping, raw| {
            assert_eq!(BYTES, 2208);
            assert_eq!((raw as usize) % core::mem::align_of::<AtomicU32>(), 0);
            assert_eq!(
                observe(mapping),
                (
                    initial(),
                    [generation as u32, (generation >> 32) as u32, 0, 0]
                )
            );
            assert!(tail_is_untouched(raw));
            for index in 0..552 {
                // SAFETY: all552 objects are initialized; isolated atomic writes.
                unsafe { &*raw.cast::<AtomicU32>().add(index) }
                    .store(index as u32 + 17, Ordering::Release);
            }
            let (prefix, guard) = observe(mapping);
            assert_eq!(prefix, core::array::from_fn(|index| index as u32 + 17));
            assert_eq!(guard, [565, 566, 567, 568]);
            assert!(tail_is_untouched(raw));
        });
    }
}

#[test]
fn combined_memory_quiescent_rearm_preserves_objects_and_replaces_every_word() {
    with_initialized(7, |mapping, raw| {
        for generation in [8, 1 << 32, (1 << 32) + 1, u64::MAX] {
            for index in 0..552 {
                // SAFETY: isolated initialized atomics, no outstanding accesses.
                unsafe { &*raw.cast::<AtomicU32>().add(index) }.store(u32::MAX, Ordering::Release);
            }
            // SAFETY: fixture has no queues/readers; this tests stores, not owner generation policy.
            unsafe {
                LinuxGfx950MemoryBackend::rearm_combined_mlp_state_v1(
                    mapping,
                    2208,
                    &initial(),
                    generation,
                )
            }
            .unwrap();
            assert_eq!(
                observe(mapping),
                (
                    initial(),
                    [generation as u32, (generation >> 32) as u32, 0, 0]
                )
            );
            assert!(tail_is_untouched(raw));
        }
    });
}

#[test]
fn combined_memory_wrong_request_refuses_before_construction_or_stores() {
    for requested in [0, 2192, 2207, 2209, 4096, usize::MAX] {
        with_fresh(|mapping, raw| {
            // SAFETY: fresh isolated storage; wrong request must refuse before effects.
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::initialize_combined_mlp_state_v1(
                        mapping,
                        requested,
                        &initial(),
                        1,
                    )
                }
                .is_err()
            );
            assert!((0..4096).all(|index| unsafe { raw.add(index).read() } == 0xa5));
        });
        with_initialized(7, |mapping, raw| {
            let before = observe(mapping);
            // SAFETY: initialized isolated storage with no device or other thread.
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::rearm_combined_mlp_state_v1(
                        mapping,
                        requested,
                        &initial(),
                        8,
                    )
                }
                .is_err()
            );
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::observe_combined_mlp_state_v1(mapping, requested)
                }
                .is_err()
            );
            assert_eq!(observe(mapping), before);
            assert!(tail_is_untouched(raw));
        });
    }
}

#[test]
fn combined_memory_bad_mapping_and_alignment_refuse_before_any_atomic_access() {
    for change in 0..4 {
        with_fresh(|mapping, raw| {
            match change {
                0 => mapping.bytes = 2207,
                1 => mapping.active = false,
                2 => mapping.accessible = false,
                _ => mapping.address = NonNull::new(raw.wrapping_add(1).cast()).unwrap(),
            }
            // SAFETY: fresh isolated storage; the checked description refuses
            // before constructing any object through the malformed pointer.
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::initialize_combined_mlp_state_v1(
                        mapping,
                        2208,
                        &initial(),
                        1,
                    )
                }
                .is_err()
            );
            assert!((0..4096).all(|index| unsafe { raw.add(index).read() } == 0xa5));
        });
        with_initialized(7, |mapping, raw| {
            let before = observe(mapping);
            let address = mapping.address;
            match change {
                0 => mapping.bytes = 2207,
                1 => mapping.active = false,
                2 => mapping.accessible = false,
                _ => mapping.address = NonNull::new(raw.wrapping_add(1).cast()).unwrap(),
            }
            // SAFETY: original objects remain alive; the malformed description
            // must be rejected before any access through its invalid range.
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::rearm_combined_mlp_state_v1(
                        mapping,
                        2208,
                        &initial(),
                        8,
                    )
                }
                .is_err()
            );
            assert!(
                unsafe { LinuxGfx950MemoryBackend::observe_combined_mlp_state_v1(mapping, 2208,) }
                    .is_err()
            );
            mapping.address = address;
            mapping.bytes = 4096;
            mapping.active = true;
            mapping.accessible = true;
            assert_eq!(observe(mapping), before);
            assert!(tail_is_untouched(raw));
        });
    }
}

#[test]
fn combined_memory_zero_generation_does_not_publish_or_mutate() {
    with_fresh(|mapping, raw| {
        // SAFETY: fresh isolated storage; zero must refuse before construction.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::initialize_combined_mlp_state_v1(
                    mapping,
                    2208,
                    &initial(),
                    0,
                )
            }
            .is_err()
        );
        assert!((0..4096).all(|index| unsafe { raw.add(index).read() } == 0xa5));
    });
    with_initialized(7, |mapping, raw| {
        let before = observe(mapping);
        // SAFETY: no outstanding fixture access; zero must refuse before stores.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::rearm_combined_mlp_state_v1(mapping, 2208, &initial(), 0)
            }
            .is_err()
        );
        assert_eq!(observe(mapping), before);
        assert!(tail_is_untouched(raw));
    });
}

#[test]
fn combined_memory_cannot_be_observed_or_rearmed_as_legacy_2192_owner() {
    with_initialized(7, |mapping, raw| {
        let before = observe(mapping);
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_wave_mlp_tile_state_v2(mapping, 2208)
                .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_mlp_tile_state_v2(mapping, 2208)
                .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::rearm_engineering_wave_mlp_tile_state_v2(mapping, 2208)
                .is_err()
        );
        assert_eq!(observe(mapping), before);
        assert!(tail_is_untouched(raw));
    });
}
