mod common;

use common::{candidate, data, metadata};
use fe2o3_qwen3_paged_gqa_decode_v1::*;

fn differential_case(
    role: Qwen3AttentionRoleV1,
    bucket: B3PagedDecodeBucketV1,
    committed_tokens: usize,
) {
    let candidate = candidate(role, bucket);
    let metadata = metadata(candidate, committed_tokens, true);
    let fixture = data(candidate, &metadata);
    let profile = candidate.profile().descriptor();
    let mut output = vec![Bf16V1::default(); fixture.query.len()];
    let state = qwen3_paged_gqa_decode_reference_v1(
        candidate,
        &metadata,
        PagedGqaInputV1 {
            query: &fixture.query,
            key: &fixture.key,
            value: &fixture.value,
        },
        &mut output,
    )
    .expect("valid fragmented paged evaluation");
    assert_eq!(
        state.output_vectors,
        profile.sequences * profile.active_tokens * profile.geometry.query_heads
    );
    assert!(state.minimum_denominator > 0.0);
    assert!(state.maximum_denominator >= state.minimum_denominator);

    for request in 0..profile.sequences {
        for local_query in 0..profile.active_tokens {
            for query_head in 0..profile.geometry.query_heads {
                let query_start = ((request * profile.active_tokens + local_query)
                    * profile.geometry.query_heads
                    + query_head)
                    * profile.geometry.head_dimension;
                let query_end = query_start + profile.geometry.head_dimension;
                let oracle = qwen3_contiguous_gqa_decode_vector_v1(
                    candidate,
                    &metadata.requests[request],
                    local_query,
                    query_head,
                    &fixture.query[query_start..query_end],
                    &fixture.contiguous_key[request],
                    &fixture.contiguous_value[request],
                )
                .expect("valid contiguous oracle");
                assert_eq!(&output[query_start..query_end], oracle.as_slice());
            }
        }
    }
}

#[test]
fn fragmented_decode_with_partial_final_page_matches_contiguous() {
    differential_case(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::DecodeS1C8192,
        18,
    );
}

#[test]
fn fragmented_speculative_draft_causal_width_matches_contiguous() {
    differential_case(
        Qwen3AttentionRoleV1::Draft06B,
        B3PagedDecodeBucketV1::SpecS1K4C8192,
        3,
    );
}

#[test]
fn page_permutation_changes_metadata_identity_not_numerical_result() {
    let candidate = candidate(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::DecodeS1C8192,
    );
    let linear = metadata(candidate, 18, false);
    let fragmented = metadata(candidate, 18, true);
    assert_ne!(
        paged_kv_metadata_identity_v1(candidate, &linear).unwrap(),
        paged_kv_metadata_identity_v1(candidate, &fragmented).unwrap()
    );
    let linear_data = data(candidate, &linear);
    let fragmented_data = data(candidate, &fragmented);
    let mut linear_output = vec![Bf16V1::default(); linear_data.query.len()];
    let mut fragmented_output = vec![Bf16V1::default(); fragmented_data.query.len()];
    qwen3_paged_gqa_decode_reference_v1(
        candidate,
        &linear,
        PagedGqaInputV1 {
            query: &linear_data.query,
            key: &linear_data.key,
            value: &linear_data.value,
        },
        &mut linear_output,
    )
    .unwrap();
    qwen3_paged_gqa_decode_reference_v1(
        candidate,
        &fragmented,
        PagedGqaInputV1 {
            query: &fragmented_data.query,
            key: &fragmented_data.key,
            value: &fragmented_data.value,
        },
        &mut fragmented_output,
    )
    .unwrap();
    assert_eq!(linear_output, fragmented_output);
}
