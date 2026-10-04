use super::*;

#[repr(C, align(64))]
struct Page([u8; 4096]);

fn mapping(page: &mut Page, bytes: usize) -> LinuxCpuMapping {
    LinuxCpuMapping {
        address: NonNull::from(page).cast(),
        bytes,
        active: true,
        accessible: true,
        reservation_phase: Arc::new(AtomicU8::new(VA_IDENTITY_MAPPED)),
    }
}

#[test]
fn initialized_atomic_rearm_preserves_exact_extents_and_tail() {
    for count in [11, 22] {
        let mut page = Page([0xa5; 4096]);
        let mut view = mapping(&mut page, count * 4);
        if count == 11 {
            LinuxGfx950MemoryBackend::initialize_engineering_wave_mlp_task_state_v1(&mut view)
                .unwrap();
        } else {
            LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_output_task_state_v5(&mut view).unwrap();
        }
        let pointer = checked_mapping_pointer(&mut view, count * 4, 0, count * 4, 4)
            .unwrap()
            .cast::<AtomicU32>();
        for index in 0..count {
            // SAFETY: this CPU-only fixture exclusively owns initialized atomics.
            unsafe { (*pointer.add(index)).store(64, Ordering::Release) };
        }
        if count == 11 {
            LinuxGfx950MemoryBackend::rearm_engineering_wave_mlp_task_state_v1(&mut view).unwrap();
            assert_eq!(
                LinuxGfx950MemoryBackend::observe_engineering_wave_mlp_task_state_v1(&mut view)
                    .unwrap(),
                core::array::from_fn(|index| u32::from(index < 2))
            );
        } else {
            LinuxGfx950MemoryBackend::rearm_engineering_wave_output_task_state_v1(&mut view)
                .unwrap();
            assert_eq!(LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_task_state_v5(&mut view).unwrap(),
                core::array::from_fn(|index| u32::from(index < 2)));
        }
        assert_eq!(&page.0[count * 4..], vec![0xa5; 4096 - count * 4]);
    }
}

#[test]
fn invalid_atomic_rearm_mapping_is_rejected_before_writes() {
    for count in [11, 22] {
        for bad in 0..4 {
            let mut page = Page([0xa5; 4096]);
            let mut view = mapping(&mut page, count * 4);
            match bad {
                0 => view.active = false,
                1 => view.accessible = false,
                2 => view.bytes -= 1,
                _ => view.address = NonNull::from(&mut page.0[1]).cast(),
            }
            let result = if count == 11 {
                LinuxGfx950MemoryBackend::rearm_engineering_wave_mlp_task_state_v1(&mut view)
            } else {
                LinuxGfx950MemoryBackend::rearm_engineering_wave_output_task_state_v1(&mut view)
            };
            assert!(result.is_err());
            assert_eq!(page.0, [0xa5; 4096]);
        }
    }
}
