mod common;

use common::candidate;
use fe2o3_qwen3_paged_gqa_decode_v1::*;

#[test]
fn exact_role_bucket_matrix_is_closed_and_bounded() {
    let mut maximum_query = 0;
    let mut maximum_kv = 0;
    let mut maximum_pairs = 0;
    for role in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ] {
        for bucket in B3_PAGED_DECODE_BUCKETS_V1 {
            let admitted = candidate(role, bucket);
            let descriptor = admitted.profile().descriptor();
            assert_eq!(descriptor.sequences, bucket.sequences());
            assert_eq!(descriptor.active_tokens, bucket.active_tokens(role));
            assert_eq!(descriptor.context_capacity_tokens, 8_192);
            assert_eq!(descriptor.page_tokens, 16);
            maximum_query = maximum_query.max(admitted.resources().query_elements);
            maximum_kv = maximum_kv.max(admitted.resources().kv_elements_each);
            maximum_pairs = maximum_pairs.max(admitted.resources().causal_pairs);
        }
    }
    assert_eq!(maximum_query, MAX_PAGED_GQA_QUERY_ELEMENTS_V1);
    assert_eq!(maximum_kv, MAX_PAGED_GQA_KV_ELEMENTS_V1);
    assert_eq!(maximum_pairs, MAX_PAGED_GQA_CAUSAL_PAIRS_V1);
}

#[test]
fn exact_active_widths_and_gqa_head_maps_are_bound_to_role() {
    let target = candidate(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::SpecS1K16C8192,
    );
    let draft = candidate(
        Qwen3AttentionRoleV1::Draft06B,
        B3PagedDecodeBucketV1::SpecS1K16C8192,
    );
    assert_eq!(target.profile().descriptor().active_tokens, 17);
    assert_eq!(draft.profile().descriptor().active_tokens, 16);
    assert_eq!(paged_gqa_kv_head_for_query_v1(target.profile(), 0), Some(0));
    assert_eq!(paged_gqa_kv_head_for_query_v1(target.profile(), 3), Some(0));
    assert_eq!(paged_gqa_kv_head_for_query_v1(target.profile(), 4), Some(1));
    assert_eq!(paged_gqa_kv_head_for_query_v1(draft.profile(), 1), Some(0));
    assert_eq!(paged_gqa_kv_head_for_query_v1(draft.profile(), 2), Some(1));
    assert_eq!(paged_gqa_kv_head_for_query_v1(draft.profile(), 16), None);
}

#[test]
fn adjacent_and_field_local_profile_mutations_reject() {
    let exact = PagedGqaProfileDescriptorV1::canonical(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::DecodeS8C8192,
    );
    let mut mutated = exact;
    mutated.sequences = 7;
    assert_eq!(
        validate_paged_gqa_profile_v1(mutated),
        Err(PagedGqaProfileErrorV1::Sequences)
    );
    let mut mutated = exact;
    mutated.active_tokens = 2;
    assert_eq!(
        validate_paged_gqa_profile_v1(mutated),
        Err(PagedGqaProfileErrorV1::ActiveTokens)
    );
    let mut mutated = exact;
    mutated.context_capacity_tokens = 8_191;
    assert_eq!(
        validate_paged_gqa_profile_v1(mutated),
        Err(PagedGqaProfileErrorV1::ContextCapacity)
    );
    let mut mutated = exact;
    mutated.page_tokens = 32;
    assert_eq!(
        validate_paged_gqa_profile_v1(mutated),
        Err(PagedGqaProfileErrorV1::PageTokens)
    );
    let mut mutated = exact;
    mutated.geometry.query_heads = 31;
    assert_eq!(
        validate_paged_gqa_profile_v1(mutated),
        Err(PagedGqaProfileErrorV1::QueryHeads)
    );
    let mut mutated = exact;
    mutated.geometry.query_heads_per_kv_head = 8;
    assert_eq!(
        validate_paged_gqa_profile_v1(mutated),
        Err(PagedGqaProfileErrorV1::GqaGroupSize)
    );
    let mut mutated = exact;
    mutated.tensor_stage = PagedGqaTensorStageV1::ProjectedBeforeQkNormAndRope;
    assert_eq!(
        validate_paged_gqa_profile_v1(mutated),
        Err(PagedGqaProfileErrorV1::TensorStage)
    );
}
