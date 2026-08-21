use fe2o3_qwen3_swiglu_v1::{
    B3_SWIGLU_BUCKETS_V1, B3SwiGluBucketV1, MAX_B3_SWIGLU_ELEMENTS_V1, Qwen3ModelRoleV1,
    SWIGLU_ELEMENTS_PER_THREAD_V1, SWIGLU_THREADS_PER_WORKGROUP_V1, SwiGluElementOwnerV1,
    SwiGluProfileDescriptorV1, SwiGluProfileErrorV1, swiglu_element_owner_v1,
    swiglu_owned_element_v1, validate_swiglu_profile_v1,
};

#[test]
fn every_exact_role_bucket_profile_is_admitted() {
    for role in [Qwen3ModelRoleV1::Target8B, Qwen3ModelRoleV1::Draft06B] {
        for bucket in B3_SWIGLU_BUCKETS_V1 {
            let descriptor = SwiGluProfileDescriptorV1::canonical(role, bucket);
            let profile = validate_swiglu_profile_v1(descriptor).unwrap();
            assert_eq!(profile.descriptor(), descriptor);
            assert_eq!(
                profile.resources().elements,
                descriptor.rows * descriptor.intermediate_size
            );
            assert_eq!(
                profile.resources().bytes_per_buffer,
                profile.resources().elements * 2
            );
            assert_eq!(
                profile.resources().global_read_bytes,
                profile.resources().bytes_per_buffer * 2
            );
            assert_eq!(
                profile.resources().global_write_bytes,
                profile.resources().bytes_per_buffer
            );
            assert_eq!(profile.resources().lds_bytes_per_workgroup, 0);
            assert_eq!(profile.resources().barriers_per_workgroup, 0);
            let coverage = usize::from(SWIGLU_THREADS_PER_WORKGROUP_V1)
                * usize::from(SWIGLU_ELEMENTS_PER_THREAD_V1);
            assert_eq!(
                profile.resources().workgroups,
                profile.resources().elements.div_ceil(coverage)
            );
        }
    }
}

#[test]
fn target_and_draft_speculative_widths_are_role_bound() {
    let target = SwiGluProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Target8B,
        B3SwiGluBucketV1::SpeculativeS8K4,
    );
    let draft = SwiGluProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Draft06B,
        B3SwiGluBucketV1::SpeculativeS8K4,
    );
    assert_eq!(
        (target.sequences, target.active_tokens, target.rows),
        (8, 5, 40)
    );
    assert_eq!(
        (draft.sequences, draft.active_tokens, draft.rows),
        (8, 4, 32)
    );
    assert_eq!(
        (target.hidden_size, target.intermediate_size),
        (4_096, 12_288)
    );
    assert_eq!((draft.hidden_size, draft.intermediate_size), (1_024, 3_072));
}

#[test]
fn maximum_profile_matches_reviewed_ceiling() {
    let descriptor = SwiGluProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Target8B,
        B3SwiGluBucketV1::PrefillS1T2048,
    );
    let profile = validate_swiglu_profile_v1(descriptor).unwrap();
    assert_eq!(profile.resources().elements, MAX_B3_SWIGLU_ELEMENTS_V1);
    assert_eq!(profile.resources().elements, 25_165_824);
}

#[test]
fn every_profile_field_is_checked_independently() {
    let exact = SwiGluProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Target8B,
        B3SwiGluBucketV1::SpeculativeS1K8,
    );

    let mut changed = exact;
    changed.sequences += 1;
    assert_eq!(
        validate_swiglu_profile_v1(changed),
        Err(SwiGluProfileErrorV1::SequenceCount)
    );

    changed = exact;
    changed.active_tokens += 1;
    assert_eq!(
        validate_swiglu_profile_v1(changed),
        Err(SwiGluProfileErrorV1::ActiveTokenCount)
    );

    changed = exact;
    changed.rows += 1;
    assert_eq!(
        validate_swiglu_profile_v1(changed),
        Err(SwiGluProfileErrorV1::FlattenedRows)
    );

    changed = exact;
    changed.hidden_size += 1;
    assert_eq!(
        validate_swiglu_profile_v1(changed),
        Err(SwiGluProfileErrorV1::HiddenSize)
    );

    changed = exact;
    changed.intermediate_size += 1;
    assert_eq!(
        validate_swiglu_profile_v1(changed),
        Err(SwiGluProfileErrorV1::IntermediateSize)
    );
}

#[test]
fn element_owner_mapping_is_exact_and_tail_owners_are_masked() {
    let profile = validate_swiglu_profile_v1(SwiGluProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Draft06B,
        B3SwiGluBucketV1::DecodeS1,
    ))
    .unwrap();
    for element in 0..profile.resources().elements {
        let owner = swiglu_element_owner_v1(profile, element).unwrap();
        assert_eq!(swiglu_owned_element_v1(profile, owner), Some(element));
    }
    assert_eq!(
        swiglu_element_owner_v1(profile, profile.resources().elements),
        None
    );
    assert_eq!(
        swiglu_owned_element_v1(
            profile,
            SwiGluElementOwnerV1 {
                workgroup: profile.resources().workgroups,
                thread: 0,
                element_in_thread: 0,
            }
        ),
        None
    );
    assert_eq!(
        swiglu_owned_element_v1(
            profile,
            SwiGluElementOwnerV1 {
                workgroup: profile.resources().workgroups - 1,
                thread: u16::MAX,
                element_in_thread: 0,
            }
        ),
        None
    );
    assert_eq!(
        swiglu_owned_element_v1(
            profile,
            SwiGluElementOwnerV1 {
                workgroup: profile.resources().workgroups - 1,
                thread: 0,
                element_in_thread: u8::MAX,
            }
        ),
        None
    );

    let tail_owner = SwiGluElementOwnerV1 {
        workgroup: profile.resources().workgroups - 1,
        thread: SWIGLU_THREADS_PER_WORKGROUP_V1 - 1,
        element_in_thread: SWIGLU_ELEMENTS_PER_THREAD_V1 - 1,
    };
    assert_eq!(swiglu_owned_element_v1(profile, tail_owner), None);
}
