use super::*;

#[repr(align(4096))]
struct Storage([u8; EXTENT]);

fn fixture(test: impl FnOnce(&mut LinuxCpuMapping, *mut u8)) {
    let mut storage = Box::<Storage>::new_uninit();
    // SAFETY: initialize the complete fresh byte-only object on the heap before
    // constructing signals; avoid a large thread-stack temporary in this test.
    unsafe { storage.as_mut_ptr().cast::<u8>().write_bytes(0xa5, EXTENT) };
    let mut storage = unsafe { storage.assume_init() };
    let address = NonNull::from(&mut *storage).cast();
    let mut mapping = LinuxCpuMapping {
        address,
        bytes: EXTENT,
        active: true,
        accessible: true,
        reservation_phase: Arc::new(AtomicU8::new(VA_IDENTITY_MAPPED)),
    };
    LinuxGfx950MemoryBackend::initialize_engineering_signal_slots(&mut mapping, 5).unwrap();
    test(&mut mapping, address.as_ptr().cast());
}

fn signals(mapping: &mut LinuxCpuMapping) -> [(i64, i64); 5] {
    core::array::from_fn(|slot| {
        LinuxGfx950MemoryBackend::observe_completion_signal_state_acquire(
            mapping,
            EXTENT,
            slot as u32,
        )
        .unwrap()
    })
}

#[test]
fn paired_reuse_memory_writes_only_four_kernarg_ranges_and_keeps_atomics_live() {
    fixture(|mapping, raw| {
        for slot in 0..5 {
            checked_completion_value(mapping, EXTENT, slot)
                .unwrap()
                .store(0, Ordering::Release);
        }
        let old = signals(mapping);
        assert!(old.iter().all(|(_, value)| *value == 0));
        let values = [
            &[1_u8, 2, 3][..],
            &[4_u8; 280][..],
            &[5_u8; 24][..],
            &[6_u8; 8][..],
        ];
        // SAFETY: isolated initialized mapping, no device or other reader.
        unsafe {
            LinuxGfx950MemoryBackend::rewrite_paired_kernargs_quiescent_v1(mapping, EXTENT, values)
        }
        .unwrap();
        assert_eq!(signals(mapping), old);
        for index in 0..4 {
            for offset in 0..SLOT {
                let expected = values[index].get(offset).copied().unwrap_or(0xa5);
                // SAFETY: this reads only the non-atomic kernarg suffix.
                assert_eq!(
                    unsafe { raw.add(SIGNAL_PAGE + index * SLOT + offset).read() },
                    expected
                );
            }
        }
        for slot in 0..5 {
            LinuxGfx950MemoryBackend::reset_completion_signal_release(mapping, EXTENT, slot)
                .unwrap();
        }
        assert_eq!(signals(mapping), old.map(|(kind, _)| (kind, 1)));
    });
}

#[test]
fn paired_reuse_memory_refuses_extent_shape_and_mapping_before_any_write() {
    fixture(|mapping, raw| {
        let values = [&[7_u8; 8][..]; 4];
        let old = signals(mapping);
        for extent in [0, SIGNAL_PAGE, EXTENT - 1, EXTENT + 1, usize::MAX] {
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::rewrite_paired_kernargs_quiescent_v1(
                        mapping, extent, values,
                    )
                }
                .is_err()
            );
        }
        let oversized = vec![3; SLOT + 1];
        for slot in 0..4 {
            let mut malformed = values;
            malformed[slot] = &oversized;
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::rewrite_paired_kernargs_quiescent_v1(
                        mapping, EXTENT, malformed,
                    )
                }
                .is_err()
            );
        }
        for state in 0..4 {
            match state {
                0 => mapping.active = false,
                1 => mapping.accessible = false,
                2 => mapping.bytes = EXTENT - 1,
                3 => mapping
                    .reservation_phase
                    .store(VA_GUARDED, Ordering::Release),
                _ => unreachable!(),
            }
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::rewrite_paired_kernargs_quiescent_v1(
                        mapping, EXTENT, values,
                    )
                }
                .is_err()
            );
            mapping.active = true;
            mapping.accessible = true;
            mapping.bytes = EXTENT;
            mapping
                .reservation_phase
                .store(VA_IDENTITY_MAPPED, Ordering::Release);
        }
        assert_eq!(signals(mapping), old);
        for offset in SIGNAL_PAGE..EXTENT {
            assert_eq!(unsafe { raw.add(offset).read() }, 0xa5);
        }
    });
}
