mod common;

use common::{PLAN, candidate};
use fe2o3_qwen3_logits_compact_v1::*;

#[test]
fn all_twenty_two_role_bucket_profiles_are_exact_bounded_and_distinct() {
    let mut identities = std::collections::BTreeSet::new();
    let mut max_rows = 0;
    let mut max_activation = 0;
    let mut max_weight = 0;
    let mut max_logits = 0;
    let mut max_work = 0;
    for role in [Qwen3LogitsRoleV1::Target8B, Qwen3LogitsRoleV1::Draft06B] {
        for bucket in B3_LOGITS_BUCKETS_V1 {
            let candidate = candidate(role, bucket);
            let profile = candidate.profile().descriptor();
            assert_eq!(profile.mode, bucket.mode());
            assert_eq!(
                profile.rows,
                bucket.sequences() * bucket.active_tokens(role)
            );
            assert_eq!(profile.speculative_k, bucket.speculative_k());
            assert!(profile.speculative_k <= 16);
            assert!(identities.insert(candidate.candidate_identity()));
            max_rows = max_rows.max(candidate.resources().rows);
            max_activation = max_activation.max(candidate.resources().activation_elements);
            max_weight = max_weight.max(candidate.resources().weight_elements);
            max_logits = max_logits.max(candidate.resources().logical_logits);
            max_work = max_work.max(candidate.resources().fp32_multiplications);
        }
    }
    assert_eq!(identities.len(), 22);
    assert_eq!(max_rows, MAX_LOGITS_ROWS_V1);
    assert_eq!(max_activation, MAX_LOGITS_ACTIVATION_ELEMENTS_V1);
    assert_eq!(max_weight, MAX_LOGITS_WEIGHT_ELEMENTS_V1);
    assert_eq!(max_logits, MAX_LOGICAL_LOGITS_V1);
    assert_eq!(max_work, MAX_LOGITS_PROJECTION_WORK_V1);
}

#[test]
fn speculative_k_and_target_extra_row_are_distinct_exact_fields() {
    let target = candidate(
        Qwen3LogitsRoleV1::Target8B,
        B3LogitsBucketV1::SpeculativeS1K16C8192,
    );
    let draft = candidate(
        Qwen3LogitsRoleV1::Draft06B,
        B3LogitsBucketV1::SpeculativeS1K16C8192,
    );
    assert_eq!(target.profile().descriptor().speculative_k, 16);
    assert_eq!(target.profile().descriptor().active_tokens, 17);
    assert_eq!(draft.profile().descriptor().speculative_k, 16);
    assert_eq!(draft.profile().descriptor().active_tokens, 16);
}

#[test]
fn descriptor_field_local_mutations_fail_closed() {
    let profile = LogitsProfileDescriptorV1::canonical(
        Qwen3LogitsRoleV1::Target8B,
        B3LogitsBucketV1::DecodeS8C8192,
    );
    let mut mutated = profile;
    mutated.mode = B3LogitsModeV1::Prefill;
    assert_eq!(
        validate_logits_profile_v1(mutated),
        Err(LogitsProfileErrorV1::Mode)
    );
    let mut mutated = profile;
    mutated.rows += 1;
    assert_eq!(
        validate_logits_profile_v1(mutated),
        Err(LogitsProfileErrorV1::Rows)
    );
    let mut mutated = profile;
    mutated.hidden_size += 1;
    assert_eq!(
        validate_logits_profile_v1(mutated),
        Err(LogitsProfileErrorV1::HiddenSize)
    );
    let mut mutated = profile;
    mutated.vocabulary_size -= 1;
    assert_eq!(
        validate_logits_profile_v1(mutated),
        Err(LogitsProfileErrorV1::VocabularySize)
    );
    let mut mutated = profile;
    mutated.speculative_k = 1;
    assert_eq!(
        validate_logits_profile_v1(mutated),
        Err(LogitsProfileErrorV1::SpeculativeK)
    );

    let exact = LogitsCandidateDescriptorV1::canonical(profile, PLAN);
    let mut mutated = exact;
    mutated.schema_version = 2;
    assert_eq!(
        admit_logits_candidate_v1(mutated),
        Err(LogitsCandidateErrorV1::SchemaVersion)
    );
    let mut mutated = exact;
    mutated.plan_identity = LogitsPlanIdentityV1([0; 32]);
    assert_eq!(
        admit_logits_candidate_v1(mutated),
        Err(LogitsCandidateErrorV1::MissingPlanIdentity)
    );
    let mut mutated = exact;
    mutated.numerical.lowest_token_id_tie_break = false;
    assert_eq!(
        admit_logits_candidate_v1(mutated),
        Err(LogitsCandidateErrorV1::Numerical(
            LogitsNumericalErrorV1::NonCanonical
        ))
    );
    let mut mutated = exact;
    mutated.effects.transactional_output = false;
    assert_eq!(
        admit_logits_candidate_v1(mutated),
        Err(LogitsCandidateErrorV1::Effects(
            LogitsEffectErrorV1::NonCanonical
        ))
    );
}
