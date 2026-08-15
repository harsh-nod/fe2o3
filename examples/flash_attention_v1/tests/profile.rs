use fe2o3_flash_attention_v1::{
    EXACT_PROFILE_V1, FLASH_ATTENTION_OUTPUT_ELEMENTS_V1, FLASH_ATTENTION_WAVE_LANES_V1, LayoutV1,
    MaskPolicyV1, ProfileMismatchV1, exact_launch_v1, key_participates_v1, lane_outputs_v1,
    qkv_index_v1, validate_profile_v1,
};

#[test]
fn exact_profile_and_launch_are_admitted() {
    assert_eq!(validate_profile_v1(EXACT_PROFILE_V1), Ok(()));
    assert_eq!(exact_launch_v1(), ([1, 1, 1], [64, 1, 1]));
}

#[test]
fn wrong_shape_layout_mask_target_launch_and_scale_are_rejected() {
    let mut profile = EXACT_PROFILE_V1;
    profile.sequence_length = 16;
    assert_eq!(validate_profile_v1(profile), Err(ProfileMismatchV1::Shape));

    let mut profile = EXACT_PROFILE_V1;
    profile.layout = LayoutV1::ColumnMajorContiguous;
    assert_eq!(validate_profile_v1(profile), Err(ProfileMismatchV1::Layout));

    let mut profile = EXACT_PROFILE_V1;
    profile.mask = MaskPolicyV1::NonCausal;
    assert_eq!(validate_profile_v1(profile), Err(ProfileMismatchV1::Mask));

    let mut profile = EXACT_PROFILE_V1;
    profile.processor = "gfx950";
    assert_eq!(validate_profile_v1(profile), Err(ProfileMismatchV1::Target));

    let mut profile = EXACT_PROFILE_V1;
    profile.workgroup = [32, 1, 1];
    assert_eq!(validate_profile_v1(profile), Err(ProfileMismatchV1::Launch));

    let mut profile = EXACT_PROFILE_V1;
    profile.attention_scale_bits ^= 1;
    assert_eq!(
        validate_profile_v1(profile),
        Err(ProfileMismatchV1::NumericalPolicy)
    );
}

#[test]
fn causal_mask_and_row_major_bounds_are_exact() {
    for query in 0..8 {
        for key in 0..8 {
            assert_eq!(key_participates_v1(query, key), key <= query);
        }
    }
    assert_eq!(qkv_index_v1(7, 15), Some(127));
    assert_eq!(qkv_index_v1(8, 0), None);
    assert_eq!(qkv_index_v1(0, 16), None);
}

#[test]
fn wave64_ownership_is_total_injective_and_has_no_tail() {
    let mut owners = [usize::MAX; FLASH_ATTENTION_OUTPUT_ELEMENTS_V1];
    for lane in 0..FLASH_ATTENTION_WAVE_LANES_V1 {
        let pair = lane_outputs_v1(lane).unwrap();
        assert_eq!(pair, [2 * lane, 2 * lane + 1]);
        for index in pair {
            assert_eq!(owners[index], usize::MAX);
            owners[index] = lane;
        }
    }
    assert!(owners.iter().all(|owner| *owner != usize::MAX));
    assert_eq!(lane_outputs_v1(FLASH_ATTENTION_WAVE_LANES_V1), None);
}
