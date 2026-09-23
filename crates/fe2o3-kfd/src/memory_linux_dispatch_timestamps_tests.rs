use super::*;

#[repr(C, align(64))]
struct Page([u8; 4096]);

fn mapping(page: &mut Page) -> LinuxCpuMapping {
    LinuxCpuMapping {
        address: NonNull::from(page).cast(),
        bytes: 4096,
        active: true,
        accessible: true,
        reservation_phase: Arc::new(AtomicU8::new(VA_IDENTITY_MAPPED)),
    }
}

#[test]
fn timestamp_properties_change_only_the_profile_bit_and_restore_exactly() {
    let mut page = Page([0xa5; 4096]);
    page.0[QUEUE_PROPERTIES_OFFSET..QUEUE_PROPERTIES_OFFSET + 4].fill(0);
    let original = page.0;
    let mut mapped = mapping(&mut page);
    // SAFETY: test-owned inert storage has no GPU references.
    let saved =
        unsafe { LinuxGfx950MemoryBackend::enable_engineering_dispatch_timestamps(&mut mapped) }
            .unwrap();
    assert_eq!(saved, 0);
    let mut expected = original;
    expected[QUEUE_PROPERTIES_OFFSET..QUEUE_PROPERTIES_OFFSET + 4]
        .copy_from_slice(&8_u32.to_le_bytes());
    assert_eq!(page.0, expected);
    // SAFETY: same inert exclusively owned test storage.
    unsafe {
        LinuxGfx950MemoryBackend::restore_engineering_dispatch_timestamps(&mut mapped, saved)
    }
    .unwrap();
    assert_eq!(page.0, original);
}

#[test]
fn timestamp_properties_reject_drift_without_mutating_any_bytes() {
    for value in [1_u32, 2, 8, 9, u32::MAX] {
        let mut page = Page([0; 4096]);
        page.0[QUEUE_PROPERTIES_OFFSET..QUEUE_PROPERTIES_OFFSET + 4]
            .copy_from_slice(&value.to_le_bytes());
        let before = page.0;
        let mut mapped = mapping(&mut page);
        // SAFETY: test-owned inert storage.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::enable_engineering_dispatch_timestamps(&mut mapped)
            }
            .is_err()
        );
        assert_eq!(page.0, before);
        if value != ENABLE_PROFILING {
            // SAFETY: test-owned inert storage; drift must be rejected.
            assert!(
                unsafe {
                    LinuxGfx950MemoryBackend::restore_engineering_dispatch_timestamps(
                        &mut mapped,
                        0,
                    )
                }
                .is_err()
            );
            assert_eq!(page.0, before);
        }
    }
}

#[test]
fn timestamp_clear_preserves_signal_value_kind_guards_and_unused_slots() {
    for count in [1, 16, 64] {
        let mut page = Page([0xa5; 4096]);
        let mut mapped = mapping(&mut page);
        // SAFETY: only inert byte storage is written, with no GPU owner.
        unsafe {
            LinuxGfx950MemoryBackend::clear_engineering_dispatch_timestamps(&mut mapped, count)
        }
        .unwrap();
        let mut expected = [0xa5; 4096];
        for slot in 0..count {
            expected[slot * 64 + 32..slot * 64 + 48].fill(0);
        }
        assert_eq!(page.0, expected);
    }
}

#[test]
fn timestamp_clear_rejects_invalid_count_mapping_and_alignment() {
    let mut page = Page([0xa5; 4096]);
    let mut mapped = mapping(&mut page);
    for count in [0, 65, usize::MAX] {
        // SAFETY: inert storage, and invalid count must fail before mutation.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::clear_engineering_dispatch_timestamps(&mut mapped, count)
            }
            .is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
    mapped.active = false;
    assert!(timestamp_slot(&mut mapped, 0).is_err());
    mapped.active = true;
    mapped.accessible = false;
    assert!(timestamp_slot(&mut mapped, 0).is_err());
    mapped.accessible = true;
    mapped.bytes = 4095;
    assert!(timestamp_slot(&mut mapped, 0).is_err());
    mapped.bytes = 4096;
    assert!(timestamp_slot(&mut mapped, 64).is_err());
    // SAFETY: this deliberately misaligned pointer is only range-checked.
    mapped.address =
        unsafe { NonNull::new_unchecked(mapped.address.as_ptr().cast::<u8>().add(1).cast()) };
    assert!(timestamp_slot(&mut mapped, 0).is_err());
    assert_eq!(page.0, [0xa5; 4096]);
}

#[test]
fn timestamp_observation_requires_completed_user_signal_and_preserves_raw_ticks() {
    let mut page = Page([0; 4096]);
    let mut mapped = mapping(&mut page);
    LinuxGfx950MemoryBackend::initialize_engineering_signal_slots64(&mut mapped, 64).unwrap();
    for slot in [0_u32, 63] {
        let start = u64::MAX - 17;
        let end = u64::MAX - 3;
        let base = slot as usize * 64;
        page.0[base + 32..base + 40].copy_from_slice(&start.to_le_bytes());
        page.0[base + 40..base + 48].copy_from_slice(&end.to_le_bytes());
        // SAFETY: inert owned signal is still pending, so observation rejects it.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::observe_engineering_dispatch_timestamps(&mut mapped, slot)
            }
            .is_err()
        );
        checked_completion_value(&mut mapped, 4096, slot)
            .unwrap()
            .store(0, Ordering::Release);
        // SAFETY: completed inert signal is retained without reuse.
        assert_eq!(
            unsafe {
                LinuxGfx950MemoryBackend::observe_engineering_dispatch_timestamps(&mut mapped, slot)
            }
            .unwrap(),
            (start, end)
        );
        page.0[base..base + 8].copy_from_slice(&0_i64.to_le_bytes());
        // SAFETY: wrong kind must reject before accepting timestamp fields.
        assert!(
            unsafe {
                LinuxGfx950MemoryBackend::observe_engineering_dispatch_timestamps(&mut mapped, slot)
            }
            .is_err()
        );
    }
}
