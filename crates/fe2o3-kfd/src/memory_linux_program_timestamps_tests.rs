use super::*;

#[repr(C, align(64))]
struct Arena([u8; 65536]);

fn mapping(arena: &mut Arena) -> LinuxCpuMapping {
    LinuxCpuMapping {
        address: NonNull::from(arena).cast(),
        bytes: 65536,
        active: true,
        accessible: true,
        reservation_phase: Arc::new(AtomicU8::new(VA_IDENTITY_MAPPED)),
    }
}

#[test]
fn native_timestamp_clear_preserves_all_other_bytes() {
    for count in [1, 64, 65, 649, 652, 688, 1024] {
        let mut arena = Arena([0xa5; 65536]);
        let mut mapped = mapping(&mut arena);
        let extent = (count * 64 + 4095) & !4095;
        // SAFETY: exclusively owned inert test storage.
        unsafe {
            LinuxGfx950MemoryBackend::clear_engineering_program_timestamps(
                &mut mapped,
                extent,
                count,
            )
        }
        .unwrap();
        for (index, byte) in arena.0.iter().enumerate() {
            let cleared = index / 64 < count && (32..48).contains(&(index % 64));
            assert_eq!(*byte, if cleared { 0 } else { 0xa5 });
        }
    }
}

#[test]
fn native_timestamp_invalid_extent_rejects_before_mutation() {
    let mut arena = Arena([0xa5; 65536]);
    let mut mapped = mapping(&mut arena);
    for (extent, count) in [
        (0, 0),
        (4096, 65),
        (8192, 64),
        (65536, 1025),
        (usize::MAX, usize::MAX),
    ] {
        // SAFETY: inert storage; invalid extent must reject before mutation.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::clear_engineering_program_timestamps(
                    &mut mapped,
                    extent,
                    count,
                )
            }
            .is_err()
        );
        assert_eq!(arena.0, [0xa5; 65536]);
    }
    mapped.bytes = 4096;
    assert!(program_extent(&mut mapped, 8192, 65).is_err());
    mapped.bytes = 65536;
    mapped.accessible = false;
    assert!(program_extent(&mut mapped, 8192, 65).is_err());
    mapped.accessible = true;
    mapped.active = false;
    assert!(program_extent(&mut mapped, 8192, 65).is_err());
    mapped.active = true;
    // SAFETY: misaligned address is only checked, never dereferenced.
    mapped.address =
        unsafe { NonNull::new_unchecked(mapped.address.as_ptr().cast::<u8>().add(1).cast()) };
    assert!(program_extent(&mut mapped, 8192, 65).is_err());
}

#[test]
fn native_timestamp_observe_requires_completed_in_range_signal() {
    let mut arena = Arena([0; 65536]);
    let mut mapped = mapping(&mut arena);
    LinuxGfx950MemoryBackend::initialize_engineering_program_signals(&mut mapped, 65536, 1024)
        .unwrap();
    for slot in [0, 64, 651, 1023] {
        let base = slot as usize * 64;
        arena.0[base + 32..base + 40].copy_from_slice(&(u64::MAX - 8).to_le_bytes());
        arena.0[base + 40..base + 48].copy_from_slice(&(u64::MAX - 2).to_le_bytes());
        // SAFETY: inert signal remains pending; acquisition must reject.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::observe_engineering_program_timestamps(
                    &mut mapped,
                    65536,
                    1024,
                    slot,
                )
            }
            .is_err()
        );
        checked_completion_value(&mut mapped, 65536, slot)
            .unwrap()
            .store(0, Ordering::Release);
        // SAFETY: completed inert signal, no concurrent reuse.
        assert_eq!(
            unsafe {
                LinuxGfx950MemoryBackend::observe_engineering_program_timestamps(
                    &mut mapped,
                    65536,
                    1024,
                    slot,
                )
            }
            .unwrap(),
            (u64::MAX - 8, u64::MAX - 2)
        );
        arena.0[base..base + 8].fill(0);
        // SAFETY: invalid kind must reject before reading timestamps.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::observe_engineering_program_timestamps(
                    &mut mapped,
                    65536,
                    1024,
                    slot,
                )
            }
            .is_err()
        );
    }
    // SAFETY: out-of-roster slot is checked before dereference.
    assert!(
        unsafe {
            LinuxGfx950MemoryBackend::observe_engineering_program_timestamps(
                &mut mapped,
                65536,
                1024,
                1024,
            )
        }
        .is_err()
    );
}
