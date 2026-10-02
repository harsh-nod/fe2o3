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
fn fragmented_decode_eight_requests_match_contiguous() {
    differential_case(
        Qwen3AttentionRoleV1::Draft06B,
        B3PagedDecodeBucketV1::DecodeS8C8192,
        1,
    );
}

#[test]
fn paged_leading_underflow_matches_contiguous_for_both_roles_and_key_orders() {
    for role in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ] {
        let candidate = candidate(role, B3PagedDecodeBucketV1::DecodeS1C8192);
        let metadata = metadata(candidate, 1, true);
        let geometry = candidate.profile().descriptor().geometry;
        let mut fixture = data(candidate, &metadata);
        fixture.query.fill(Bf16V1::default());
        for query in fixture.query.chunks_exact_mut(geometry.head_dimension) {
            query[0] = common::bf16(1.0);
        }
        for maximum_key in [0, 1] {
            for token in 0..2 {
                for kv_head in 0..geometry.kv_heads {
                    for feature in 0..geometry.head_dimension {
                        let key = common::bf16(if feature == 0 && token != maximum_key {
                            -2048.0
                        } else {
                            0.0
                        });
                        let value = common::bf16(if token == maximum_key { 3.0 } else { 7.0 });
                        let physical = common::physical_index(
                            candidate, &metadata, 0, token, kv_head, feature,
                        );
                        let logical = (token * geometry.kv_heads + kv_head)
                            * geometry.head_dimension
                            + feature;
                        fixture.key[physical] = key;
                        fixture.value[physical] = value;
                        fixture.contiguous_key[0][logical] = key;
                        fixture.contiguous_value[0][logical] = value;
                    }
                }
            }
            let mut output = vec![common::bf16(-9.0); fixture.query.len()];
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
            .unwrap();
            assert_eq!(state.minimum_denominator, 1.0);
            assert_eq!(state.maximum_denominator, 1.0);
            assert_eq!(output, vec![common::bf16(3.0); fixture.query.len()]);
            let oracle = qwen3_contiguous_gqa_decode_vector_v1(
                candidate,
                &metadata.requests[0],
                0,
                0,
                &fixture.query[..geometry.head_dimension],
                &fixture.contiguous_key[0],
                &fixture.contiguous_value[0],
            )
            .unwrap();
            assert_eq!(&output[..geometry.head_dimension], oracle.as_slice());
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
