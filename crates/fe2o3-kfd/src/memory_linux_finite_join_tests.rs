use super::*;

#[test]
fn engineering_mlp_v1_atomic_state_exact_extent_and_acquire_readback() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    mapping.bytes = 44;
    LinuxGfx950MemoryBackend::initialize_engineering_wave_mlp_task_state_v1(&mut mapping).unwrap();
    let mut expected = [0; 11];
    expected[..2].copy_from_slice(&[1, 1]);
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_mlp_task_state_v1(&mut mapping).unwrap(),
        expected
    );
    let pointer = checked_mapping_pointer(&mut mapping, 44, 0, 44, 4)
        .unwrap()
        .cast::<AtomicU32>();
    expected = [64; 11];
    expected[..6].copy_from_slice(&[1, 0, 31, 31, 0x2aa, 0]);
    for (index, value) in expected.into_iter().enumerate() {
        // SAFETY: the CPU-only fixture exclusively owns initialized atomics.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_mlp_task_state_v1(&mut mapping).unwrap(),
        expected
    );
    assert!(page.0[44..].iter().all(|&byte| byte == 0xa5));
}

#[test]
fn engineering_mlp_v1_atomic_mapping_refusals_precede_all_writes() {
    for mutation in 0..7 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 40,
            3 => mapping.bytes = 43,
            4 => mapping.bytes = 0,
            5 => {
                mapping.address = NonNull::from(&mut page.0[2]).cast();
                mapping.bytes = 4094;
            }
            _ => mapping.bytes = 36,
        }
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_wave_mlp_task_state_v1(&mut mapping)
                .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_mlp_task_state_v1(&mut mapping)
                .is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}

#[test]
fn engineering_output_v5_atomic_state_exact_extent_and_high_owner_readback() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    mapping.bytes = 88;
    LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_output_task_state_v5(
        &mut mapping,
    )
    .unwrap();
    let mut expected = [0; 22];
    expected[..2].copy_from_slice(&[1, 1]);
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_task_state_v5(
            &mut mapping
        )
        .unwrap(),
        expected
    );
    let pointer = checked_mapping_pointer(&mut mapping, 88, 0, 88, 4)
        .unwrap()
        .cast::<AtomicU32>();
    expected = [64; 22];
    expected[..6].copy_from_slice(&[1, 0, 65535, 65535, 0xaaaa_aaaa, 0]);
    for (index, value) in expected.into_iter().enumerate() {
        // SAFETY: the CPU-only fixture exclusively owns these initialized atomics.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_task_state_v5(
            &mut mapping
        )
        .unwrap(),
        expected
    );
    assert!(page.0[88..].iter().all(|&byte| byte == 0xa5));
}

#[test]
fn engineering_output_v5_atomic_mapping_refusals_precede_all_writes() {
    for mutation in 0..7 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 84,
            3 => mapping.bytes = 87,
            4 => mapping.bytes = 0,
            5 => {
                mapping.address = NonNull::from(&mut page.0[2]).cast();
                mapping.bytes = 4094;
            }
            _ => mapping.bytes = 36,
        }
        assert!(LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_output_task_state_v5(&mut mapping).is_err());
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_output_task_state_v5(
                &mut mapping
            )
            .is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}

#[test]
fn engineering_attention_v4_atomic_state_exact_extent_and_readback() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    mapping.bytes = 84;
    LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_task_state_v4(&mut mapping)
        .unwrap();
    let mut expected = [0; 21];
    expected[..2].copy_from_slice(&[1, 1]);
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_task_state_v4(
            &mut mapping
        )
        .unwrap(),
        expected
    );
    let pointer = checked_mapping_pointer(&mut mapping, 84, 0, 84, 4)
        .unwrap()
        .cast::<AtomicU32>();
    expected = [64; 21];
    expected[..6].copy_from_slice(&[1, 0, 32767, 32767, 0x1555_5555, 0]);
    for (index, value) in expected.into_iter().enumerate() {
        // SAFETY: this CPU-only test exclusively owns the initialized atomics.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_task_state_v4(
            &mut mapping
        )
        .unwrap(),
        expected
    );
    assert!(page.0[84..].iter().all(|&byte| byte == 0xa5));
}
#[test]
fn engineering_attention_v4_atomic_mapping_refusals_precede_all_writes() {
    for mutation in 0..7 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 80,
            3 => mapping.bytes = 83,
            4 => mapping.bytes = 0,
            5 => {
                mapping.address = NonNull::from(&mut page.0[1]).cast();
                mapping.bytes = 4095;
            }
            _ => mapping.bytes = 36,
        }
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_attention_task_state_v4(
                &mut mapping
            )
            .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_attention_task_state_v4(
                &mut mapping
            )
            .is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}

#[test]
fn engineering_post_v3_atomic_state_exact_extent_and_readback() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    mapping.bytes = 80;
    LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_post_task_state_v3(&mut mapping)
        .unwrap();
    let mut expected = [0; 20];
    expected[..2].copy_from_slice(&[1, 1]);
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_post_task_state_v3(&mut mapping)
            .unwrap(),
        expected
    );
    let pointer = checked_mapping_pointer(&mut mapping, 80, 0, 80, 4)
        .unwrap()
        .cast::<AtomicU32>();
    expected = [64; 20];
    expected[..6].copy_from_slice(&[1, 0, 16383, 16383, 0x0555_5555, 0]);
    for (index, value) in expected.into_iter().enumerate() {
        // SAFETY: this CPU-only test exclusively owns the initialized atomics.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_post_task_state_v3(&mut mapping)
            .unwrap(),
        expected
    );
    assert!(page.0[80..].iter().all(|&byte| byte == 0xa5));
}
#[test]
fn engineering_post_v3_atomic_mapping_refusals_precede_all_writes() {
    for mutation in 0..7 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 76,
            3 => mapping.bytes = 79,
            4 => mapping.bytes = 0,
            5 => {
                mapping.address = NonNull::from(&mut page.0[1]).cast();
                mapping.bytes = 4095;
            }
            _ => mapping.bytes = 36,
        }
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_post_task_state_v3(
                &mut mapping
            )
            .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_post_task_state_v3(&mut mapping)
                .is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}

#[test]
fn engineering_wave_qkv_task_atomic_initialization_and_readback_are_exact() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_task_state_v2(&mut mapping).unwrap();
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_task_state_v2(&mut mapping).unwrap(),
        [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(page.0[76..].iter().all(|byte| *byte == 0xa5));
    let pointer = checked_mapping_pointer(&mut mapping, 76, 0, 76, 4)
        .unwrap()
        .cast::<AtomicU32>();
    let completed = [
        1,
        0,
        8191,
        8191,
        0x0155_5555,
        0,
        64,
        64,
        64,
        64,
        64,
        64,
        64,
        64,
        64,
        64,
        64,
        64,
        64,
    ];
    for (index, value) in completed.into_iter().enumerate() {
        // SAFETY: this test owns initialized atomic storage; no GPU is involved.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_task_state_v2(&mut mapping).unwrap(),
        completed
    );
    assert!(page.0[76..].iter().all(|byte| *byte == 0xa5));
}

#[test]
fn engineering_wave_qkv_task_atomic_mapping_rejections_precede_all_writes() {
    for mutation in 0..6 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 36,
            3 => mapping.bytes = 75,
            4 => mapping.bytes = 0,
            5 => {
                mapping.address = NonNull::from(&mut page.0[1]).cast();
                mapping.bytes = 4095;
            }
            _ => unreachable!(),
        }
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_task_state_v2(&mut mapping)
                .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_task_state_v2(&mut mapping)
                .is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}

#[test]
fn engineering_wave_qkv_task_initializer_does_not_touch_the_next_atomic_word() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    mapping.bytes = 76;
    LinuxGfx950MemoryBackend::initialize_engineering_wave_qkv_task_state_v2(&mut mapping).unwrap();
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_qkv_task_state_v2(&mut mapping).unwrap(),
        [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(page.0[76..].iter().all(|byte| *byte == 0xa5));
}

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
fn engineering_finite_join_atomic_initialization_and_readback_are_exact() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    LinuxGfx950MemoryBackend::initialize_engineering_finite_join_state(&mut mapping).unwrap();
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_finite_join_state(&mut mapping).unwrap(),
        [1, 3, 0, 0, 0, 0]
    );
    assert!(page.0[24..].iter().all(|byte| *byte == 0xa5));
    let pointer = checked_mapping_pointer(&mut mapping, 24, 0, 24, 4)
        .unwrap()
        .cast::<AtomicU32>();
    for (index, value) in [1, 0, 7, 7, 21, 0].into_iter().enumerate() {
        // SAFETY: this CPU test owns the exact initialized atomic array. No GPU.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_finite_join_state(&mut mapping).unwrap(),
        [1, 0, 7, 7, 21, 0]
    );
    assert!(page.0[24..].iter().all(|byte| *byte == 0xa5));
}

#[test]
fn engineering_finite_join_atomic_mapping_rejections_precede_all_writes() {
    for mutation in 0..4 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 23,
            3 => {
                mapping.address = NonNull::from(&mut page.0[1]).cast();
                mapping.bytes = 4095;
            }
            _ => unreachable!(),
        }
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_finite_join_state(&mut mapping)
                .is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_finite_join_state(&mut mapping).is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}

#[test]
fn engineering_wave_task_atomic_initialization_and_readback_are_exact() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    LinuxGfx950MemoryBackend::initialize_engineering_wave_task_state(&mut mapping).unwrap();
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_task_state(&mut mapping).unwrap(),
        [1, 1, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(page.0[36..].iter().all(|byte| *byte == 0xa5));
    let pointer = checked_mapping_pointer(&mut mapping, 36, 0, 36, 4)
        .unwrap()
        .cast::<AtomicU32>();
    let completed = [1, 0, 7, 7, 0b10_01_01, 0, 64, 64, 64];
    for (index, value) in completed.into_iter().enumerate() {
        // SAFETY: this test owns initialized atomic storage; no GPU is involved.
        unsafe { (*pointer.add(index)).store(value, Ordering::Release) };
    }
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_task_state(&mut mapping).unwrap(),
        completed
    );
    assert!(page.0[36..].iter().all(|byte| *byte == 0xa5));
}

#[test]
fn engineering_wave_task_atomic_mapping_rejections_precede_all_writes() {
    for mutation in 0..6 {
        let mut page = Page([0xa5; 4096]);
        let mut mapping = mapping(&mut page);
        match mutation {
            0 => mapping.active = false,
            1 => mapping.accessible = false,
            2 => mapping.bytes = 24,
            3 => mapping.bytes = 35,
            4 => mapping.bytes = 0,
            5 => {
                mapping.address = NonNull::from(&mut page.0[1]).cast();
                mapping.bytes = 4095;
            }
            _ => unreachable!(),
        }
        assert!(
            LinuxGfx950MemoryBackend::initialize_engineering_wave_task_state(&mut mapping).is_err()
        );
        assert!(
            LinuxGfx950MemoryBackend::observe_engineering_wave_task_state(&mut mapping).is_err()
        );
        assert_eq!(page.0, [0xa5; 4096]);
    }
}

#[test]
fn engineering_wave_task_initializer_does_not_touch_the_next_atomic_word() {
    let mut page = Page([0xa5; 4096]);
    let mut mapping = mapping(&mut page);
    mapping.bytes = 36;
    LinuxGfx950MemoryBackend::initialize_engineering_wave_task_state(&mut mapping).unwrap();
    assert_eq!(
        LinuxGfx950MemoryBackend::observe_engineering_wave_task_state(&mut mapping).unwrap(),
        [1, 1, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(page.0[36..].iter().all(|byte| *byte == 0xa5));
}
