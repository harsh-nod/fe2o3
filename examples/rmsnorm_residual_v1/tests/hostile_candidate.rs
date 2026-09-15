use std::collections::BTreeSet;

use fe2o3_rmsnorm_residual_v1::{
    B3_RMSNORM_BUCKETS_V1, B3RmsNormBucketV1, OutputCastPolicyV1, Qwen3ModelRoleV1,
    ReciprocalRootPolicyV1, ResidualAddPolicyV1, RmsNormCandidateDescriptorV1,
    RmsNormProfileDescriptorV1, ScaleOrderPolicyV1, SquareReductionPolicyV1,
    validate_structural_candidate_v1,
};

fn canonical() -> RmsNormCandidateDescriptorV1 {
    RmsNormCandidateDescriptorV1::canonical(RmsNormProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Target8B,
        B3RmsNormBucketV1::DecodeS1,
    ))
}

fn rejects(descriptor: RmsNormCandidateDescriptorV1) {
    assert!(validate_structural_candidate_v1(descriptor).is_err());
}

#[test]
fn every_profile_and_algorithm_mutation_fails_closed() {
    let mut wrong = canonical();
    wrong.schema_version += 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.algorithm = "qwen3-bf16-rmsnorm";
    rejects(wrong);

    let mut wrong = canonical();
    wrong.profile.role = Qwen3ModelRoleV1::Draft06B;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.profile.bucket = B3RmsNormBucketV1::DecodeS8;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.profile.sequences += 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.profile.active_tokens += 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.profile.rows += 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.profile.hidden_size -= 1;
    rejects(wrong);
}

#[test]
fn every_numerical_policy_mutation_fails_closed() {
    let mut wrong = canonical();
    wrong.numerical.epsilon_bits ^= 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.numerical.residual_add = ResidualAddPolicyV1::Bf16Add;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.numerical.square_reduction = SquareReductionPolicyV1::SequentialFp32;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.numerical.reciprocal_root = ReciprocalRootPolicyV1::ApproximateRsqrt;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.numerical.scale_order = ScaleOrderPolicyV1::WeightFirst;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.numerical.output_cast = OutputCastPolicyV1::Bf16Truncate;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.numerical.reject_non_finite = false;
    rejects(wrong);
}

#[test]
fn every_effect_contract_mutation_fails_closed() {
    let mut wrong = canonical();
    wrong.effects.initialized_read_buffers -= 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.effects.write_buffers -= 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.effects.read_only_inputs_may_alias = false;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.effects.writable_outputs_are_disjoint = false;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.effects.output_mapping_is_total_and_injective = false;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.effects.wave_collectives_are_convergent = false;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.effects.output_commit_is_transactional = false;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.effects.accesses_are_bounded = false;
    rejects(wrong);
}

#[test]
fn every_schedule_mutation_fails_closed() {
    let mut wrong = canonical();
    wrong.schedule.schema_version += 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.processor = "gfx950";
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.target_features = "+wavefrontsize32,-xnack";
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.rows_per_workgroup += 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.lanes_per_wave /= 2;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.waves_per_workgroup += 1;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.column_stride /= 2;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.reduction_strides.swap(0, 1);
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.lds_bytes = 256;
    rejects(wrong);

    let mut wrong = canonical();
    wrong.schedule.output_owners_per_element += 1;
    rejects(wrong);
}

#[test]
fn all_role_bucket_candidates_have_unique_bound_identities() {
    let mut algorithms = BTreeSet::new();
    let mut schedules = BTreeSet::new();
    let mut candidates = BTreeSet::new();
    for role in [Qwen3ModelRoleV1::Target8B, Qwen3ModelRoleV1::Draft06B] {
        for bucket in B3_RMSNORM_BUCKETS_V1 {
            let descriptor = RmsNormCandidateDescriptorV1::canonical(
                RmsNormProfileDescriptorV1::canonical(role, bucket),
            );
            let first = validate_structural_candidate_v1(descriptor).unwrap();
            let second = validate_structural_candidate_v1(descriptor).unwrap();
            assert_eq!(first, second);
            assert!(!first.grants_production_authority());
            algorithms.insert(first.algorithm_identity());
            schedules.insert(first.schedule_identity());
            candidates.insert(first.candidate_identity());
        }
    }
    assert_eq!(algorithms.len(), 22);
    assert_eq!(schedules.len(), 22);
    assert_eq!(candidates.len(), 22);
}

#[test]
fn canonical_target_decode_identity_is_golden() {
    let candidate = validate_structural_candidate_v1(canonical()).unwrap();
    assert_eq!(
        candidate.algorithm_identity().bytes(),
        [
            0xee, 0xf2, 0x32, 0x19, 0xba, 0x2b, 0x4d, 0x21, 0x88, 0xea, 0x6b, 0xa7, 0xfd, 0x11,
            0x36, 0xe6, 0x79, 0x5f, 0x7c, 0xe0, 0xd3, 0x1e, 0x30, 0xc7, 0x02, 0x78, 0x7e, 0x1f,
            0x18, 0x8a, 0x72, 0x6e,
        ]
    );
    assert_eq!(
        candidate.schedule_identity().bytes(),
        [
            0xe0, 0x27, 0xa3, 0xee, 0xdc, 0x9e, 0x18, 0x01, 0x32, 0x0a, 0xf0, 0xfd, 0x1b, 0x17,
            0xf0, 0x1e, 0x66, 0xfe, 0x98, 0x3f, 0x76, 0x72, 0x21, 0xbc, 0xac, 0x22, 0x40, 0xf0,
            0xd2, 0x05, 0x6f, 0xcc,
        ]
    );
    assert_eq!(
        candidate.candidate_identity().bytes(),
        [
            0xbe, 0xa5, 0x2a, 0x8a, 0x56, 0xe2, 0x3e, 0x1d, 0x49, 0x01, 0xad, 0xb3, 0xdd, 0x9f,
            0xd6, 0xa9, 0x0d, 0x17, 0x47, 0x93, 0xa7, 0xca, 0x2f, 0x02, 0xbe, 0x6f, 0x94, 0x6e,
            0x65, 0xee, 0x9a, 0xf5,
        ]
    );
}
