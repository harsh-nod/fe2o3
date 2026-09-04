use std::collections::BTreeSet;

use fe2o3_qwen3_gqa_prefill_v1::{
    AttentionOutputCastV1, B3_PREFILL_BUCKETS_V1, B3PrefillBucketV1, CausalPolicyV1,
    ExponentialPolicyV1, GqaCandidateDescriptorV1, GqaHeadMappingV1, GqaPrefillProfileDescriptorV1,
    GqaTensorStageV1, KvLayoutV1, QueryLayoutV1, Qwen3AttentionRoleV1, ScorePolicyV1,
    SoftmaxPolicyV1, ValuePolicyV1, VectorOrderV1, validate_structural_gqa_candidate_v1,
};

fn canonical() -> GqaCandidateDescriptorV1 {
    GqaCandidateDescriptorV1::canonical(GqaPrefillProfileDescriptorV1::canonical(
        Qwen3AttentionRoleV1::Target8B,
        B3PrefillBucketV1::S1T128,
    ))
}

fn rejects(descriptor: GqaCandidateDescriptorV1) {
    assert!(validate_structural_gqa_candidate_v1(descriptor).is_err());
}

#[test]
fn every_candidate_and_profile_field_mutation_fails_closed() {
    let mut wrong = canonical();
    wrong.schema_version += 1;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.algorithm = "qwen3-gqa-prefill";
    rejects(wrong);
    let mut wrong = canonical();
    wrong.profile.role = Qwen3AttentionRoleV1::Draft06B;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.profile.bucket = B3PrefillBucketV1::S1T512;
    rejects(wrong);

    let profile_mutations: &[fn(&mut GqaCandidateDescriptorV1)] = &[
        |value| value.profile.sequences += 1,
        |value| value.profile.active_tokens += 1,
        |value| value.profile.context_tokens += 1,
        |value| value.profile.geometry.hidden_size -= 1,
        |value| value.profile.geometry.query_heads -= 1,
        |value| value.profile.geometry.kv_heads -= 1,
        |value| value.profile.geometry.head_dimension -= 1,
        |value| value.profile.geometry.query_heads_per_kv_head -= 1,
        |value| value.profile.geometry.query_projection_size -= 1,
        |value| value.profile.geometry.kv_projection_size -= 1,
        |value| value.profile.tensor_stage = GqaTensorStageV1::ProjectedBeforeQkNormAndRope,
    ];
    for mutate in profile_mutations {
        let mut wrong = canonical();
        mutate(&mut wrong);
        rejects(wrong);
    }
}

#[test]
fn every_numerical_field_mutation_fails_closed() {
    let mut wrong = canonical();
    wrong.numerical.attention_scale_bits ^= 1;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.causal = CausalPolicyV1::Unmasked;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.score = ScorePolicyV1::ScaleBeforeReduction;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.softmax = SoftmaxPolicyV1::OnlineFp32;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.exponential = ExponentialPolicyV1::OcmlExpF32;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.value = ValuePolicyV1::ContractedFma;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.output_cast = AttentionOutputCastV1::Bf16Truncate;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.reject_non_finite_inputs = false;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.reject_non_finite_intermediates = false;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.numerical.allow_exponential_underflow = false;
    rejects(wrong);
}

#[test]
fn every_effect_field_mutation_fails_closed() {
    let mutations: &[fn(&mut GqaCandidateDescriptorV1)] = &[
        |value| value.effects.initialized_read_buffers -= 1,
        |value| value.effects.write_buffers += 1,
        |value| value.effects.read_only_inputs_may_alias = false,
        |value| value.effects.output_is_disjoint = false,
        |value| value.effects.output_mapping_is_total_and_injective = false,
        |value| value.effects.independent_vectors_are_race_free = false,
        |value| value.effects.reads_are_causal_only = false,
        |value| value.effects.accesses_are_bounded = false,
        |value| value.effects.output_commit_is_transactional = false,
    ];
    for mutate in mutations {
        let mut wrong = canonical();
        mutate(&mut wrong);
        rejects(wrong);
    }
}

#[test]
fn every_evaluation_field_mutation_fails_closed() {
    let mut wrong = canonical();
    wrong.evaluation.schema_version += 1;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.query_output_layout = QueryLayoutV1::SequenceHeadTokenFeature;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.key_value_layout = KvLayoutV1::SequenceHeadTokenFeature;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.vector_order = VectorOrderV1::SequenceQueryHeadTokenAscending;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.head_mapping = GqaHeadMappingV1::Modulo;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.causal_keys_ascending = false;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.qk_features_ascending = false;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.output_features_ascending = false;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.token_scratch_arrays -= 1;
    rejects(wrong);
    let mut wrong = canonical();
    wrong.evaluation.separate_output_staging = false;
    rejects(wrong);
}

#[test]
fn all_eight_role_bucket_identities_are_unique_and_deterministic() {
    let mut algorithms = BTreeSet::new();
    let mut evaluations = BTreeSet::new();
    let mut candidates = BTreeSet::new();
    for role in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ] {
        for bucket in B3_PREFILL_BUCKETS_V1 {
            let descriptor = GqaCandidateDescriptorV1::canonical(
                GqaPrefillProfileDescriptorV1::canonical(role, bucket),
            );
            let first = validate_structural_gqa_candidate_v1(descriptor).unwrap();
            let second = validate_structural_gqa_candidate_v1(descriptor).unwrap();
            assert_eq!(first, second);
            assert!(!first.grants_production_authority());
            algorithms.insert(first.algorithm_identity());
            evaluations.insert(first.evaluation_identity());
            candidates.insert(first.candidate_identity());
        }
    }
    assert_eq!(algorithms.len(), 8);
    assert_eq!(evaluations.len(), 8);
    assert_eq!(candidates.len(), 8);
}

#[test]
fn canonical_target_s1t128_identity_is_golden() {
    let candidate = validate_structural_gqa_candidate_v1(canonical()).unwrap();
    assert_eq!(
        candidate.algorithm_identity().bytes(),
        [
            0x76, 0xce, 0xa0, 0xd2, 0x7c, 0xc0, 0xda, 0x64, 0xff, 0xbe, 0x05, 0x46, 0x4c, 0x92,
            0x00, 0x4a, 0xd5, 0x2c, 0xa1, 0x64, 0xb2, 0x30, 0x35, 0xa1, 0x4e, 0xe1, 0xc8, 0x06,
            0x0b, 0xc4, 0x9f, 0x45,
        ]
    );
    assert_eq!(
        candidate.evaluation_identity().bytes(),
        [
            0xcc, 0x5c, 0xd6, 0x95, 0xa5, 0x2f, 0xc2, 0xf4, 0x35, 0xc6, 0xae, 0x5a, 0x35, 0x5e,
            0x07, 0xb3, 0x32, 0xdc, 0xda, 0x8f, 0x00, 0x6f, 0x48, 0xdb, 0x12, 0x4b, 0xc3, 0x07,
            0xa7, 0xe7, 0xdc, 0x10,
        ]
    );
    assert_eq!(
        candidate.candidate_identity().bytes(),
        [
            0x65, 0x7b, 0x4b, 0x71, 0xf9, 0x58, 0x44, 0xe1, 0x2d, 0x14, 0x48, 0x14, 0xbb, 0x76,
            0xd4, 0x41, 0x23, 0x57, 0xc2, 0x44, 0x7b, 0x71, 0xc1, 0x1d, 0x8b, 0x68, 0xf2, 0x2c,
            0xe2, 0x96, 0x6a, 0x79,
        ]
    );
}
