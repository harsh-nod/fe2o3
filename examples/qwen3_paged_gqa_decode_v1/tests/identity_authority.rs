use fe2o3_qwen3_paged_gqa_decode_v1::*;

#[test]
fn every_candidate_field_is_fail_closed_and_identity_is_deterministic() {
    let profile = PagedGqaProfileDescriptorV1::canonical(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::SpecS8K4C8192,
    );
    let exact = PagedGqaCandidateDescriptorV1::canonical(profile);
    let first = admit_paged_gqa_decode_candidate_v1(exact).unwrap();
    let second = admit_paged_gqa_decode_candidate_v1(exact).unwrap();
    assert_eq!(first.candidate_identity(), second.candidate_identity());
    assert_ne!(first.algorithm_identity(), first.evaluation_identity());

    let mut mutated = exact;
    mutated.schema_version = 2;
    assert_eq!(
        admit_paged_gqa_decode_candidate_v1(mutated),
        Err(PagedGqaCandidateErrorV1::SchemaVersion)
    );
    let mut mutated = exact;
    mutated.algorithm = "paged-attention";
    assert_eq!(
        admit_paged_gqa_decode_candidate_v1(mutated),
        Err(PagedGqaCandidateErrorV1::Algorithm)
    );
    let mut mutated = exact;
    mutated.numerical.logical_keys_ascending = false;
    assert_eq!(
        admit_paged_gqa_decode_candidate_v1(mutated),
        Err(PagedGqaCandidateErrorV1::Numerical(
            PagedGqaNumericalErrorV1::NonCanonical
        ))
    );
    let mut mutated = exact;
    mutated.effects.final_page_mask_enforced = false;
    assert_eq!(
        admit_paged_gqa_decode_candidate_v1(mutated),
        Err(PagedGqaCandidateErrorV1::Effects(
            PagedGqaEffectErrorV1::NonCanonical
        ))
    );
    let mut mutated = exact;
    mutated.evaluation.head_mapping = PagedGqaHeadMappingV1::Modulo;
    assert_eq!(
        admit_paged_gqa_decode_candidate_v1(mutated),
        Err(PagedGqaCandidateErrorV1::Evaluation(
            PagedGqaEvaluationErrorV1::NonCanonical
        ))
    );
    let mut mutated = exact;
    mutated.evaluation.query_position = PagedQueryPositionPolicyV1::ResidentPlusLocalToken;
    assert_eq!(
        admit_paged_gqa_decode_candidate_v1(mutated),
        Err(PagedGqaCandidateErrorV1::Evaluation(
            PagedGqaEvaluationErrorV1::NonCanonical
        ))
    );
}

#[test]
fn role_and_every_b3_bucket_have_distinct_candidate_identities() {
    let mut identities = std::collections::BTreeSet::new();
    for role in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ] {
        for bucket in B3_PAGED_DECODE_BUCKETS_V1 {
            let profile = PagedGqaProfileDescriptorV1::canonical(role, bucket);
            let identity = admit_paged_gqa_decode_candidate_v1(
                PagedGqaCandidateDescriptorV1::canonical(profile),
            )
            .unwrap()
            .candidate_identity();
            assert!(identities.insert(identity));
        }
    }
    assert_eq!(identities.len(), 14);
}

#[test]
fn all_production_authorities_remain_closed() {
    assert!(!std::hint::black_box(
        PAGED_GQA_DECODE_SOURCE_TO_KIR_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        PAGED_GQA_DECODE_VERUS_PROOF_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        PAGED_GQA_DECODE_ARTIFACT_PUBLICATION_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        PAGED_GQA_DECODE_ARTIFACT_LOAD_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(PAGED_GQA_DECODE_LAUNCH_SUPPORTED_V1));
    assert!(!std::hint::black_box(
        PAGED_GQA_DECODE_MACHINE_REFINEMENT_PROVED_V1
    ));
    assert!(PAGED_GQA_DECODE_PRODUCTION_BLOCKER_V1.contains("Rust MIR authority join"));
}
