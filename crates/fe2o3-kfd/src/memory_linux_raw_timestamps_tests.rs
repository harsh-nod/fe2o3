use super::*;
use fe2o3_aql::{
    AMD_SIGNAL_END_TIMESTAMP_OFFSET_V1 as END, AMD_SIGNAL_START_TIMESTAMP_OFFSET_V1 as START,
    AmdBusyCompletionSignalV1,
};

fn with_signal(f: impl FnOnce(&mut LinuxCpuMapping)) {
    let mut signal = AmdBusyCompletionSignalV1::new_pending();
    let mut mapping = LinuxCpuMapping {
        address: NonNull::from(&mut signal).cast(),
        bytes: AMD_SIGNAL_BYTES_V1,
        active: true,
        accessible: true,
        reservation_phase: Arc::new(AtomicU8::new(VA_IDENTITY_MAPPED)),
    };
    f(&mut mapping);
}

fn write_word(mapping: &mut LinuxCpuMapping, offset: usize, value: u64) {
    let pointer = checked_mapping_pointer(mapping, 64, offset, 8, 8).unwrap();
    // SAFETY: CPU-only fixture has no GPU/thread writer; these are plain fields.
    unsafe { pointer.cast::<u64>().write_volatile(value) };
}

fn complete(mapping: &mut LinuxCpuMapping) {
    checked_completion_value(mapping, 64, 0)
        .unwrap()
        .store(0, Ordering::Release);
}

#[test]
fn raw_signal_initial_arm_clears_both_fields_before_pending() {
    with_signal(|mapping| {
        write_word(mapping, START, 17);
        write_word(mapping, END, 25);
        // SAFETY: CPU-only fixture, never published.
        unsafe { LinuxGfx950MemoryBackend::arm_raw_completion_timestamps(mapping, 64, true) }
            .unwrap();
        assert_eq!(
            LinuxGfx950MemoryBackend::observe_completion_signal_acquire(mapping, 64, 0).unwrap(),
            AqlCompletionObservationV1::Pending
        );
        complete(mapping);
        assert_eq!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .unwrap(),
            [0, 0]
        );
    });
}

#[test]
fn raw_signal_reuse_requires_complete_then_discards_prior_ticks() {
    with_signal(|mapping| {
        write_word(mapping, START, 17);
        write_word(mapping, END, 25);
        assert!(
            unsafe { LinuxGfx950MemoryBackend::arm_raw_completion_timestamps(mapping, 64, false) }
                .is_err()
        );
        complete(mapping);
        assert_eq!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .unwrap(),
            [17, 25]
        );
        unsafe { LinuxGfx950MemoryBackend::arm_raw_completion_timestamps(mapping, 64, false) }
            .unwrap();
        assert!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .is_err()
        );
        complete(mapping);
        assert_eq!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .unwrap(),
            [0, 0]
        );
    });
}

#[test]
fn raw_signal_requires_exact_kind_and_acquired_zero() {
    for value in [1, -1, i64::MAX] {
        with_signal(|mapping| {
            checked_completion_value(mapping, 64, 0)
                .unwrap()
                .store(value, Ordering::Release);
            assert!(
                unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                    .is_err()
            );
        });
    }
    with_signal(|mapping| {
        complete(mapping);
        write_word(mapping, 0, 0);
        assert!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .is_err()
        );
        assert!(
            unsafe { LinuxGfx950MemoryBackend::arm_raw_completion_timestamps(mapping, 64, false) }
                .is_err()
        );
    });
}

#[test]
fn raw_signal_preserves_64_bit_ticks_without_conversion() {
    with_signal(|mapping| {
        write_word(mapping, START, 0x1234_5678_90ab_cdef);
        write_word(mapping, END, 0xfedc_ba98_7654_3210);
        complete(mapping);
        let expected = [0x1234_5678_90ab_cdef, 0xfedc_ba98_7654_3210];
        assert_eq!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .unwrap(),
            expected
        );
        assert_eq!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .unwrap(),
            expected
        );
    });
}

#[test]
fn raw_signal_rejects_wrong_extent_and_alignment() {
    with_signal(|mapping| {
        complete(mapping);
        for bytes in [0, 63, 65, usize::MAX] {
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, bytes)
                }
                .is_err()
            );
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::arm_raw_completion_timestamps(mapping, bytes, false)
                }
                .is_err()
            );
        }
        let original = mapping.address;
        mapping.address = NonNull::new(original.as_ptr().wrapping_add(1)).unwrap();
        assert!(
            unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                .is_err()
        );
        mapping.address = original;
    });
}

#[test]
fn raw_signal_rejects_inactive_inaccessible_and_retired_mapping() {
    for mode in 0..3 {
        with_signal(|mapping| {
            complete(mapping);
            match mode {
                0 => mapping.active = false,
                1 => mapping.accessible = false,
                _ => mapping.reservation_phase.store(0, Ordering::Release),
            }
            assert!(
                unsafe { LinuxGfx950MemoryBackend::capture_raw_completion_timestamps(mapping, 64) }
                    .is_err()
            );
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::arm_raw_completion_timestamps(mapping, 64, false)
                }
                .is_err()
            );
        });
    }
}
