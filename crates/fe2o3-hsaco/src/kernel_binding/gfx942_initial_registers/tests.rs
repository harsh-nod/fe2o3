use super::*;

fn descriptor() -> AmdhsaKernelDescriptor {
    AmdhsaKernelDescriptor {
        group_segment_fixed_size: 0,
        private_segment_fixed_size: 0,
        kernarg_size: 16,
        kernel_code_entry_byte_offset: 256,
        compute_pgm_rsrc3: 0,
        compute_pgm_rsrc1: 1 << 6,
        compute_pgm_rsrc2: (2 << 1) | (1 << 7),
        kernel_code_properties: 1 << 3,
        kernarg_preload: 0,
    }
}

#[test]
fn all_supported_enable_combinations_have_exact_dense_nonoverlapping_locations() {
    for user_mask in 0u16..16 {
        for system_mask in 0u32..16 {
            for workitem_mode in 0..3 {
                let mut descriptor = descriptor();
                descriptor.kernel_code_properties = user_mask << 1;
                let users = user_mask.count_ones() * 2;
                descriptor.compute_pgm_rsrc2 =
                    (users << 1) | (system_mask << 7) | (workitem_mode << 11);
                let layout = derive(descriptor).unwrap();
                let named_pairs = [
                    layout.dispatch_pointer_sgprs(),
                    layout.queue_pointer_sgprs(),
                    layout.kernarg_pointer_sgprs(),
                    layout.dispatch_id_sgprs(),
                ];
                let mut expected = Vec::new();
                for (bit, actual) in named_pairs.into_iter().enumerate() {
                    if user_mask & (1 << bit) != 0 {
                        let low = expected.len() as u8;
                        assert_eq!(actual, Some([low, low + 1]));
                        assert_eq!(layout.user_pairs[bit], Some([low, low + 1]));
                        expected.extend([low, low + 1]);
                    } else {
                        assert_eq!(actual, None);
                        assert_eq!(layout.user_pairs[bit], None);
                    }
                }
                assert_eq!(u32::from(layout.user_sgpr_count()), users);
                for bit in 0..4 {
                    let location = if bit < 3 {
                        layout.workgroup_id_sgprs()[bit]
                    } else {
                        layout.workgroup_info_sgpr()
                    };
                    if system_mask & (1 << bit) != 0 {
                        assert_eq!(location, Some(expected.len() as u8));
                        expected.push(expected.len() as u8);
                    } else {
                        assert_eq!(location, None);
                    }
                }
                assert_eq!(usize::from(layout.initialized_sgpr_count()), expected.len());
                assert_eq!(
                    expected,
                    (0..layout.initialized_sgpr_count()).collect::<Vec<_>>()
                );
                let ids = layout.workitem_id_vgprs();
                for (index, actual) in ids.into_iter().enumerate() {
                    assert_eq!(
                        actual,
                        (index as u32 <= workitem_mode).then_some(index as u8)
                    );
                }
                assert_eq!(
                    u32::from(layout.initialized_vgpr_count()),
                    workitem_mode + 1
                );
            }
        }
    }
}

#[test]
fn every_nonexact_user_count_rejects_without_shifting_system_registers() {
    for mask in 0u16..16 {
        for actual in 0u8..32 {
            let mut descriptor = descriptor();
            descriptor.kernel_code_properties = mask << 1;
            descriptor.compute_pgm_rsrc2 = (u32::from(actual) << 1) | (1 << 7);
            let expected = mask.count_ones() as u8 * 2;
            let result = derive(descriptor);
            if actual == expected {
                assert_eq!(
                    result.unwrap().workgroup_id_sgprs(),
                    [Some(expected), None, None]
                );
            } else {
                assert_eq!(
                    result,
                    Err(Gfx942InitialRegisterLayoutErrorV1::UserSgprCount { expected, actual })
                );
            }
        }
    }
}

#[test]
fn private_inputs_preload_wave32_and_undefined_workitem_mode_reject() {
    use Gfx942InitialRegisterLayoutErrorV1 as E;
    for bit in [0, 5, 6, 11] {
        let mut descriptor = descriptor();
        descriptor.kernel_code_properties |= 1 << bit;
        assert_eq!(derive(descriptor), Err(E::PrivateInputsOrScratch));
    }
    let mut private_enabled = descriptor();
    private_enabled.compute_pgm_rsrc2 |= 1;
    assert_eq!(derive(private_enabled), Err(E::PrivateInputsOrScratch));
    let mut private_size = descriptor();
    private_size.private_segment_fixed_size = 4;
    assert_eq!(derive(private_size), Err(E::PrivateInputsOrScratch));
    for preload in [1, 64, u16::MAX] {
        let mut descriptor = descriptor();
        descriptor.kernarg_preload = preload;
        assert_eq!(derive(descriptor), Err(E::KernargPreload));
    }
    let mut wave32 = descriptor();
    wave32.kernel_code_properties |= 1 << 10;
    assert_eq!(derive(wave32), Err(E::WavefrontSize));
    let mut workitem = descriptor();
    workitem.compute_pgm_rsrc2 |= 3 << 11;
    assert_eq!(derive(workitem), Err(E::WorkitemIdMode));
}

#[test]
fn initial_locations_must_fit_encoded_sgpr_capacity() {
    let mut descriptor = descriptor();
    descriptor.kernel_code_properties = 0x1e;
    descriptor.compute_pgm_rsrc2 = 8 << 1;
    descriptor.compute_pgm_rsrc1 = 0;
    assert_eq!(derive(descriptor).unwrap().initialized_sgpr_count(), 8);
    descriptor.compute_pgm_rsrc2 |= 1 << 7;
    assert_eq!(
        derive(descriptor),
        Err(Gfx942InitialRegisterLayoutErrorV1::InsufficientSgprCapacity)
    );
    descriptor.compute_pgm_rsrc1 = 1 << 6;
    assert_eq!(derive(descriptor).unwrap().initialized_sgpr_count(), 9);
}

#[test]
fn requested_user_inputs_shift_kernarg_and_workgroup_locations() {
    let base = derive(descriptor()).unwrap();
    assert_eq!(base.kernarg_pointer_sgprs(), Some([0, 1]));
    assert_eq!(base.workgroup_id_sgprs(), [Some(2), None, None]);
    let mut shifted = descriptor();
    shifted.kernel_code_properties = 0x1e;
    shifted.compute_pgm_rsrc2 = (8 << 1) | (1 << 7);
    let shifted = derive(shifted).unwrap();
    assert_eq!(shifted.dispatch_pointer_sgprs(), Some([0, 1]));
    assert_eq!(shifted.queue_pointer_sgprs(), Some([2, 3]));
    assert_eq!(shifted.kernarg_pointer_sgprs(), Some([4, 5]));
    assert_eq!(shifted.dispatch_id_sgprs(), Some([6, 7]));
    assert_eq!(shifted.workgroup_id_sgprs(), [Some(8), None, None]);
}
