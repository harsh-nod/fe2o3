use fe2o3_qwen3_gqa_prefill_v1::{
    B3_PREFILL_BUCKETS_V1, B3PrefillBucketV1, GqaPrefillProfileDescriptorV1, GqaProfileErrorV1,
    GqaTensorStageV1, MAX_GQA_CAUSAL_PAIRS_V1, MAX_GQA_KV_ELEMENTS_V1, MAX_GQA_QUERY_ELEMENTS_V1,
    Qwen3AttentionRoleV1, Qwen3GqaGeometryV1, gqa_key_participates_v1, gqa_kv_head_for_query_v1,
    gqa_kv_index_v1, gqa_query_index_v1, validate_gqa_prefill_profile_v1,
};

type ProfileMutation = (fn(&mut GqaPrefillProfileDescriptorV1), GqaProfileErrorV1);

#[test]
fn exact_eight_role_bucket_profiles_and_resources_are_admitted() {
    for role in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ] {
        for bucket in B3_PREFILL_BUCKETS_V1 {
            let descriptor = GqaPrefillProfileDescriptorV1::canonical(role, bucket);
            let profile = validate_gqa_prefill_profile_v1(descriptor).unwrap();
            let resources = profile.resources();
            assert_eq!(descriptor.sequences, bucket.sequences());
            assert_eq!(descriptor.active_tokens, bucket.tokens());
            assert_eq!(descriptor.context_tokens, bucket.tokens());
            assert_eq!(resources.output_elements, resources.query_elements);
            assert_eq!(
                resources.input_payload_bytes,
                (resources.query_elements + 2 * resources.kv_elements_each) * 2
            );
            assert_eq!(resources.output_payload_bytes, resources.query_elements * 2);
            assert_eq!(
                resources.vector_scratch_bytes,
                descriptor.active_tokens as u64 * 8
            );
            assert_eq!(
                resources.transactional_scratch_bytes,
                resources.output_payload_bytes + resources.vector_scratch_bytes
            );
            assert_eq!(
                resources.qk_multiplications,
                resources.causal_pairs * descriptor.geometry.head_dimension as u64
            );
            assert_eq!(
                resources.value_multiplications,
                resources.qk_multiplications
            );
            assert_eq!(resources.exponential_evaluations, resources.causal_pairs);
            assert_eq!(resources.output_divisions, resources.output_elements);
            assert!(resources.query_elements <= MAX_GQA_QUERY_ELEMENTS_V1);
            assert!(resources.kv_elements_each <= MAX_GQA_KV_ELEMENTS_V1);
            assert!(resources.causal_pairs <= MAX_GQA_CAUSAL_PAIRS_V1);
        }
    }
}

#[test]
fn target_and_draft_geometry_are_exact_and_intentionally_different() {
    assert_eq!(
        Qwen3GqaGeometryV1::exact(Qwen3AttentionRoleV1::Target8B),
        Qwen3GqaGeometryV1 {
            hidden_size: 4_096,
            query_heads: 32,
            kv_heads: 8,
            head_dimension: 128,
            query_heads_per_kv_head: 4,
            query_projection_size: 4_096,
            kv_projection_size: 1_024,
        }
    );
    assert_eq!(
        Qwen3GqaGeometryV1::exact(Qwen3AttentionRoleV1::Draft06B),
        Qwen3GqaGeometryV1 {
            hidden_size: 1_024,
            query_heads: 16,
            kv_heads: 8,
            head_dimension: 128,
            query_heads_per_kv_head: 2,
            query_projection_size: 2_048,
            kv_projection_size: 1_024,
        }
    );
}

#[test]
fn maximum_target_profile_pins_exact_quadratic_ceiling() {
    let profile = validate_gqa_prefill_profile_v1(GqaPrefillProfileDescriptorV1::canonical(
        Qwen3AttentionRoleV1::Target8B,
        B3PrefillBucketV1::S1T2048,
    ))
    .unwrap();
    let resources = profile.resources();
    assert_eq!(resources.query_elements, 8_388_608);
    assert_eq!(resources.kv_elements_each, 2_097_152);
    assert_eq!(resources.causal_pairs, 67_141_632);
    assert_eq!(resources.qk_multiplications, 8_594_128_896);
    assert_eq!(resources.value_multiplications, 8_594_128_896);
    assert_eq!(resources.exponential_evaluations, 67_141_632);
    assert_eq!(resources.output_divisions, 8_388_608);
    assert_eq!(resources.input_payload_bytes, 25_165_824);
    assert_eq!(resources.output_payload_bytes, 16_777_216);
    assert_eq!(resources.vector_scratch_bytes, 16_384);
}

#[test]
fn adjacent_and_cross_role_profiles_fail_closed() {
    let canonical = GqaPrefillProfileDescriptorV1::canonical(
        Qwen3AttentionRoleV1::Target8B,
        B3PrefillBucketV1::S1T128,
    );
    let mutations: &[ProfileMutation] = &[
        (|value| value.sequences += 1, GqaProfileErrorV1::Sequences),
        (
            |value| value.active_tokens += 1,
            GqaProfileErrorV1::ActiveTokens,
        ),
        (
            |value| value.context_tokens += 1,
            GqaProfileErrorV1::ContextTokens,
        ),
        (
            |value| value.geometry.hidden_size -= 1,
            GqaProfileErrorV1::HiddenSize,
        ),
        (
            |value| value.geometry.query_heads -= 1,
            GqaProfileErrorV1::QueryHeads,
        ),
        (
            |value| value.geometry.kv_heads -= 1,
            GqaProfileErrorV1::KvHeads,
        ),
        (
            |value| value.geometry.head_dimension -= 1,
            GqaProfileErrorV1::HeadDimension,
        ),
        (
            |value| value.geometry.query_heads_per_kv_head -= 1,
            GqaProfileErrorV1::GqaGroupSize,
        ),
        (
            |value| value.geometry.query_projection_size -= 1,
            GqaProfileErrorV1::QueryProjection,
        ),
        (
            |value| value.geometry.kv_projection_size -= 1,
            GqaProfileErrorV1::KvProjection,
        ),
        (
            |value| value.tensor_stage = GqaTensorStageV1::ProjectedBeforeQkNormAndRope,
            GqaProfileErrorV1::TensorStage,
        ),
    ];
    for (mutate, expected) in mutations {
        let mut wrong = canonical;
        mutate(&mut wrong);
        assert_eq!(validate_gqa_prefill_profile_v1(wrong), Err(*expected));
    }

    let mut wrong = canonical;
    wrong.role = Qwen3AttentionRoleV1::Draft06B;
    assert!(validate_gqa_prefill_profile_v1(wrong).is_err());
    let mut wrong = canonical;
    wrong.bucket = B3PrefillBucketV1::S1T512;
    assert!(validate_gqa_prefill_profile_v1(wrong).is_err());
}

#[test]
fn layouts_causal_domain_and_gqa_mapping_are_checked() {
    for role in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ] {
        let profile = validate_gqa_prefill_profile_v1(GqaPrefillProfileDescriptorV1::canonical(
            role,
            B3PrefillBucketV1::S8T128,
        ))
        .unwrap();
        let descriptor = profile.descriptor();
        let geometry = descriptor.geometry;
        for query_head in 0..geometry.query_heads {
            assert_eq!(
                gqa_kv_head_for_query_v1(profile, query_head),
                Some(query_head / geometry.query_heads_per_kv_head)
            );
        }
        assert_eq!(
            gqa_kv_head_for_query_v1(profile, geometry.query_heads),
            None
        );
        assert_eq!(gqa_query_index_v1(profile, 0, 0, 0, 0), Some(0));
        assert_eq!(gqa_kv_index_v1(profile, 0, 0, 0, 0), Some(0));
        assert_eq!(
            gqa_query_index_v1(
                profile,
                descriptor.sequences - 1,
                descriptor.active_tokens - 1,
                geometry.query_heads - 1,
                geometry.head_dimension - 1,
            ),
            Some(profile.resources().query_elements as usize - 1)
        );
        assert_eq!(
            gqa_query_index_v1(profile, descriptor.sequences, 0, 0, 0),
            None
        );
        assert_eq!(
            gqa_kv_index_v1(profile, 0, descriptor.active_tokens, 0, 0),
            None
        );
        for query_token in 0..descriptor.active_tokens {
            assert!(gqa_key_participates_v1(profile, query_token, query_token));
            if query_token + 1 < descriptor.active_tokens {
                assert!(!gqa_key_participates_v1(
                    profile,
                    query_token,
                    query_token + 1
                ));
            }
        }
    }
}
