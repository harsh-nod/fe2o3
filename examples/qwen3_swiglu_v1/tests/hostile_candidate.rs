use fe2o3_qwen3_swiglu_v1::{
    B3SwiGluBucketV1, Qwen3ModelRoleV1, SwiGluBufferBindingV1, SwiGluCandidateDescriptorV1,
    SwiGluCandidateErrorV1, SwiGluProfileDescriptorV1, SwiGluScheduleDescriptorV1,
    swiglu_identity_bundle_v1, validate_swiglu_candidate_v1, validate_swiglu_profile_v1,
};
use std::fmt::Write as _;

fn descriptor() -> SwiGluCandidateDescriptorV1 {
    let profile = SwiGluProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Draft06B,
        B3SwiGluBucketV1::DecodeS1,
    );
    let bytes = u64::try_from(
        validate_swiglu_profile_v1(profile)
            .unwrap()
            .resources()
            .bytes_per_buffer,
    )
    .unwrap();
    SwiGluCandidateDescriptorV1 {
        profile,
        gate: SwiGluBufferBindingV1 {
            allocation_id: 1,
            generation: 7,
            byte_offset: 0,
            byte_len: bytes,
        },
        up: SwiGluBufferBindingV1 {
            allocation_id: 2,
            generation: 7,
            byte_offset: 0,
            byte_len: bytes,
        },
        activated: SwiGluBufferBindingV1 {
            allocation_id: 3,
            generation: 7,
            byte_offset: 0,
            byte_len: bytes,
        },
        schedule: SwiGluScheduleDescriptorV1::canonical(),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

#[test]
fn exact_candidate_is_admitted_and_identities_are_deterministic() {
    let first = validate_swiglu_candidate_v1(descriptor()).unwrap();
    let second = validate_swiglu_candidate_v1(descriptor()).unwrap();
    let first_ids = swiglu_identity_bundle_v1(first);
    let second_ids = swiglu_identity_bundle_v1(second);
    assert_eq!(first_ids, second_ids);
    for identity in [
        first_ids.profile,
        first_ids.algorithm,
        first_ids.schedule,
        first_ids.candidate,
    ] {
        assert_ne!(identity.as_bytes(), &[0; 32]);
    }
    assert_eq!(
        hex(first_ids.profile.as_bytes()),
        "07f2636c63ffa70a5e251f0744658a867fe1c83129a6c325415272c712d560eb"
    );
    assert_eq!(
        hex(first_ids.algorithm.as_bytes()),
        "6a0a8f3c4a0f9b7b1f6e287e4cf3c6ea7062e5f8d06aa66251608fc815ddfd66"
    );
    assert_eq!(
        hex(first_ids.schedule.as_bytes()),
        "f111e2e1bd10400c40dfe0c9b2e5777f01c633a5e3a4d9632ff642c05f0e0d7a"
    );
    assert_eq!(
        hex(first_ids.candidate.as_bytes()),
        "eee6b23e33c8305e4013967dcd43a686a24904e6b4765996b245664e502cf04a"
    );
}

#[test]
fn absent_misaligned_wrong_length_and_overflow_buffers_fail_closed() {
    let exact = descriptor();

    let mut changed = exact;
    changed.gate.allocation_id = 0;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::AbsentBufferAuthority)
    );

    changed = exact;
    changed.up.generation = 0;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::AbsentBufferAuthority)
    );

    changed = exact;
    changed.activated.byte_offset = 1;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::MisalignedBuffer)
    );

    changed = exact;
    changed.gate.byte_len -= 2;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::BufferLength)
    );

    changed = exact;
    changed.up.byte_offset = u64::MAX - 1;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::BufferRangeOverflow)
    );
}

#[test]
fn every_pairwise_alias_is_rejected() {
    let exact = descriptor();
    for pair in [(0, 1), (0, 2), (1, 2)] {
        let mut changed = exact;
        let source = match pair.0 {
            0 => exact.gate,
            1 => exact.up,
            _ => exact.activated,
        };
        match pair.1 {
            1 => changed.up = source,
            2 => changed.activated = source,
            _ => unreachable!(),
        }
        assert_eq!(
            validate_swiglu_candidate_v1(changed),
            Err(SwiGluCandidateErrorV1::BufferOverlap)
        );
    }
}

#[test]
fn adjacent_same_allocation_regions_are_admitted() {
    let mut changed = descriptor();
    let bytes = changed.gate.byte_len;
    changed.up.allocation_id = changed.gate.allocation_id;
    changed.up.generation = changed.gate.generation;
    changed.up.byte_offset = bytes;
    changed.activated.allocation_id = changed.gate.allocation_id;
    changed.activated.generation = changed.gate.generation;
    changed.activated.byte_offset = bytes * 2;
    validate_swiglu_candidate_v1(changed).unwrap();
}

#[test]
fn every_schedule_axis_is_exact() {
    let exact = descriptor();

    let mut changed = exact;
    changed.schedule.threads_per_workgroup -= 1;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::Schedule)
    );

    changed = exact;
    changed.schedule.elements_per_thread += 1;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::Schedule)
    );

    changed = exact;
    changed.schedule.lds_bytes_per_workgroup = 2;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::Schedule)
    );

    changed = exact;
    changed.schedule.barriers_per_workgroup = 1;
    assert_eq!(
        validate_swiglu_candidate_v1(changed),
        Err(SwiGluCandidateErrorV1::Schedule)
    );
}

#[test]
fn runtime_generation_and_binding_changes_rekey_only_the_candidate() {
    let first = swiglu_identity_bundle_v1(validate_swiglu_candidate_v1(descriptor()).unwrap());
    let mut changed = descriptor();
    changed.gate.generation += 1;
    let second = swiglu_identity_bundle_v1(validate_swiglu_candidate_v1(changed).unwrap());
    assert_eq!(first.profile, second.profile);
    assert_eq!(first.algorithm, second.algorithm);
    assert_eq!(first.schedule, second.schedule);
    assert_ne!(first.candidate, second.candidate);
}

#[test]
fn valid_role_and_bucket_changes_rekey_shape_schedule_and_candidate() {
    let first = swiglu_identity_bundle_v1(validate_swiglu_candidate_v1(descriptor()).unwrap());
    let mut changed = descriptor();
    changed.profile = SwiGluProfileDescriptorV1::canonical(
        Qwen3ModelRoleV1::Target8B,
        B3SwiGluBucketV1::DecodeS8,
    );
    let bytes = u64::try_from(
        validate_swiglu_profile_v1(changed.profile)
            .unwrap()
            .resources()
            .bytes_per_buffer,
    )
    .unwrap();
    changed.gate.byte_len = bytes;
    changed.up.byte_len = bytes;
    changed.activated.byte_len = bytes;
    let second = swiglu_identity_bundle_v1(validate_swiglu_candidate_v1(changed).unwrap());
    assert_ne!(first.profile, second.profile);
    assert_eq!(first.algorithm, second.algorithm);
    assert_ne!(first.schedule, second.schedule);
    assert_ne!(first.candidate, second.candidate);
}
