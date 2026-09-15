use fe2o3_rmsnorm_residual_v1::{
    B3_RMSNORM_BUCKETS_V1, B3RmsNormBucketV1, MAX_B3_RMS_ELEMENTS_V1, Qwen3ModelRoleV1,
    RMSNORM_WAVE_LANES_V1, RmsNormProfileDescriptorV1, RmsNormProfileErrorV1,
    rmsnorm_element_index_v1, rmsnorm_lane_owns_column_v1, validate_rmsnorm_profile_v1,
};

#[test]
fn exact_b3_matrix_is_closed_and_role_derived() {
    let target_rows = [128, 1_024, 512, 2_048, 1, 8, 32, 5, 40, 9, 17];
    let draft_rows = [128, 1_024, 512, 2_048, 1, 8, 32, 4, 32, 8, 16];
    for (role, expected_rows) in [
        (Qwen3ModelRoleV1::Target8B, target_rows),
        (Qwen3ModelRoleV1::Draft06B, draft_rows),
    ] {
        for (index, bucket) in B3_RMSNORM_BUCKETS_V1.into_iter().enumerate() {
            let descriptor = RmsNormProfileDescriptorV1::canonical(role, bucket);
            let profile = validate_rmsnorm_profile_v1(descriptor).unwrap();
            assert_eq!(descriptor.rows, expected_rows[index]);
            assert_eq!(
                descriptor.rows,
                descriptor.sequences * descriptor.active_tokens
            );
            assert_eq!(descriptor.hidden_size, role.hidden_size());
            assert_eq!(profile.resources().workgroups, descriptor.rows);
            assert_eq!(profile.resources().waves, descriptor.rows);
            assert_eq!(profile.resources().threads_per_workgroup, 64);
            assert_eq!(profile.resources().lds_bytes_per_workgroup, 0);
            assert!(profile.resources().activation_elements <= MAX_B3_RMS_ELEMENTS_V1);
            assert_eq!(
                profile.resources().global_read_bytes,
                profile.resources().activation_elements * 6
            );
            assert_eq!(
                profile.resources().global_write_bytes,
                profile.resources().activation_elements * 4
            );
            assert_eq!(
                profile.resources().host_scratch_bytes,
                profile.resources().activation_elements * 4 + descriptor.hidden_size * 4
            );
        }
    }
}

#[test]
fn speculative_target_and_draft_active_token_counts_are_distinct() {
    let cases = [
        (B3RmsNormBucketV1::SpeculativeS1K4, 5, 4),
        (B3RmsNormBucketV1::SpeculativeS8K4, 5, 4),
        (B3RmsNormBucketV1::SpeculativeS1K8, 9, 8),
        (B3RmsNormBucketV1::SpeculativeS1K16, 17, 16),
    ];
    for (bucket, target, draft) in cases {
        assert_eq!(bucket.active_tokens(Qwen3ModelRoleV1::Target8B), target);
        assert_eq!(bucket.active_tokens(Qwen3ModelRoleV1::Draft06B), draft);
    }
}

#[test]
fn adjacent_or_cross_role_shape_records_fail_closed() {
    let canonical = RmsNormProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Target8B,
        B3RmsNormBucketV1::DecodeS8,
    );

    let mut wrong = canonical;
    wrong.sequences += 1;
    assert_eq!(
        validate_rmsnorm_profile_v1(wrong),
        Err(RmsNormProfileErrorV1::SequenceCount)
    );

    let mut wrong = canonical;
    wrong.active_tokens += 1;
    assert_eq!(
        validate_rmsnorm_profile_v1(wrong),
        Err(RmsNormProfileErrorV1::ActiveTokenCount)
    );

    let mut wrong = canonical;
    wrong.rows += 1;
    assert_eq!(
        validate_rmsnorm_profile_v1(wrong),
        Err(RmsNormProfileErrorV1::FlattenedRows)
    );

    let mut wrong = canonical;
    wrong.hidden_size = 1_024;
    assert_eq!(
        validate_rmsnorm_profile_v1(wrong),
        Err(RmsNormProfileErrorV1::HiddenSize)
    );

    let mut wrong = canonical;
    wrong.role = Qwen3ModelRoleV1::Draft06B;
    assert!(validate_rmsnorm_profile_v1(wrong).is_err());
}

#[test]
fn checked_index_and_lane_ownership_are_total_and_injective() {
    let profile = validate_rmsnorm_profile_v1(RmsNormProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Draft06B,
        B3RmsNormBucketV1::SpeculativeS1K4,
    ))
    .unwrap();
    let descriptor = profile.descriptor();

    for row in 0..descriptor.rows {
        let mut owner_count = vec![0_u8; descriptor.hidden_size];
        for lane in 0..RMSNORM_WAVE_LANES_V1 {
            for (column, owners) in owner_count.iter_mut().enumerate() {
                if rmsnorm_lane_owns_column_v1(lane, column) {
                    *owners += 1;
                    assert_eq!(
                        rmsnorm_element_index_v1(profile, row, column),
                        Some(row * descriptor.hidden_size + column)
                    );
                }
            }
        }
        assert!(owner_count.into_iter().all(|owners| owners == 1));
    }
    assert_eq!(rmsnorm_element_index_v1(profile, descriptor.rows, 0), None);
    assert_eq!(
        rmsnorm_element_index_v1(profile, 0, descriptor.hidden_size),
        None
    );
    assert!(!rmsnorm_lane_owns_column_v1(64, 0));
}
