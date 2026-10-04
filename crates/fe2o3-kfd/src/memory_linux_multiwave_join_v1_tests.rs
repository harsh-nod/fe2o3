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
fn engineering_multiwave_join_atomic_initialization_and_readback_are_exact() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    LinuxGfx950MemoryBackend::initialize_engineering_multiwave_join_state(&mut mapping).unwrap();
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_multiwave_join_state(&mut mapping).unwrap(),
        [1, 3, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(page.0[36..].iter().all(|byte| *byte == 0xa5));
    let pointer = checked_mapping_pointer(&mut mapping, 36, 0, 36, 4)
        .unwrap()
        .cast::<AtomicU32>();
    for (index, value) in [1, 0, 7, 7, 21, 0, 128, 128, 128].into_iter().enumerate() {
        // SAFETY: this CPU test owns the exact initialized atomic array. No GPU.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_multiwave_join_state(&mut mapping).unwrap(),
        [1, 0, 7, 7, 21, 0, 128, 128, 128]
    );
    assert!(page.0[36..].iter().all(|byte| *byte == 0xa5));
}

#[test]
fn engineering_multiwave_join_atomic_mapping_rejections_precede_all_writes() {
    for mutation in 0..5 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 35,
            3 => {
                mapping.address = NonNull::from(&mut page.0[1]).cast();
                mapping.bytes = 4095;
            }
            4 => mapping.bytes = 24,
            _ => unreachable!(),
        }
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_multiwave_join_state(&mut mapping)
                .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_multiwave_join_state(&mut mapping)
                .is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}
