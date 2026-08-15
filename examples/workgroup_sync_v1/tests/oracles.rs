use fe2o3_workgroup_sync_v1::{
    AtomicAddressSpaceV1, AtomicLaneV1, AtomicOrderingV1, AtomicProfileErrorV1, AtomicScopeV1,
    ReductionProfileErrorV1, atomic_add_oracle_v1, atomic_vectors_v1, canonical_atomic_lanes_v1,
    canonical_atomic_profile_v1, canonical_reduction_trace_v1, compare_atomic_output_v1,
    compare_reduction_output_v1, lds_reduction_oracle_v1, reduction_vectors_v1,
};

fn reduction_rejects_without_mutation(
    values: &[i32],
    epoch: u32,
    trace: &[fe2o3_workgroup_sync_v1::ReductionLaneV1],
) -> ReductionProfileErrorV1 {
    let mut output = [0x1357_2468_i32];
    let error = lds_reduction_oracle_v1(values, epoch, trace, &mut output).unwrap_err();
    assert_eq!(output, [0x1357_2468]);
    error
}

fn atomic_rejects_without_mutation(
    profile: fe2o3_workgroup_sync_v1::AtomicProfileV1,
    lanes: &[AtomicLaneV1],
) -> AtomicProfileErrorV1 {
    let mut output = [0x1357_2468_u32];
    let error = atomic_add_oracle_v1(7, profile, lanes, &mut output).unwrap_err();
    assert_eq!(output, [0x1357_2468]);
    error
}

#[test]
fn deterministic_reduction_vectors_compute_exact_sums() {
    for vector in reduction_vectors_v1() {
        let trace = canonical_reduction_trace_v1(vector.epoch);
        let mut output = [i32::MIN];
        let expected = lds_reduction_oracle_v1(&vector.values, vector.epoch, &trace, &mut output)
            .unwrap_or_else(|error| panic!("{} was rejected: {error:?}", vector.name));
        assert_eq!(expected, vector.expected, "{}", vector.name);
        assert_eq!(output, [vector.expected], "{}", vector.name);
        assert_eq!(compare_reduction_output_v1(expected, output[0]), Ok(()));
    }
}

#[test]
fn reduction_rejects_lane_shape_epoch_barrier_and_ownership_mutations() {
    let values = [1_i32; 64];
    let epoch = 9;
    let trace = canonical_reduction_trace_v1(epoch);

    assert!(matches!(
        reduction_rejects_without_mutation(&values[..63], epoch, &trace),
        ReductionProfileErrorV1::InvalidLaneCount { provided: 63 }
    ));
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &trace[..63]),
        ReductionProfileErrorV1::InvalidTraceCount { provided: 63 }
    ));

    let mut mutation = trace.clone();
    mutation[63].lane = 64;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::LaneOutOfRange { lane: 64 }
    ));

    let mut mutation = trace.clone();
    mutation[63].lane = 62;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::DuplicateLane { lane: 62 }
    ));

    let mut mutation = trace.clone();
    mutation[11].publish_slot = 12;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::WrongPublishSlot { lane: 11, slot: 12 }
    ));

    let mut mutation = trace.clone();
    mutation[7].publish_epoch = epoch - 1;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::WrongPublishEpoch { lane: 7, .. }
    ));

    let mut mutation = trace.clone();
    mutation[8].read_epoch = epoch - 1;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::StaleReadEpoch { lane: 8, .. }
    ));

    let mut mutation = trace.clone();
    mutation[3].publish_barrier = None;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::MissingPublishBarrier { lane: 3 }
    ));

    let mut mutation = trace.clone();
    mutation[4].publish_barrier = Some(99);
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::DivergentPublishBarrier { lane: 4, .. }
    ));

    let mut mutation = trace.clone();
    mutation[5].reuse_barrier = None;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::MissingReuseBarrier { lane: 5 }
    ));

    let mut mutation = trace.clone();
    mutation[6].reuse_barrier = Some(100);
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::DivergentReuseBarrier { lane: 6, .. }
    ));

    let mut mutation = trace.clone();
    mutation[1].writes_output = true;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::DuplicateOutputWriter {
            first: 0,
            second: 1
        }
    ));

    let mut mutation = trace;
    mutation[0].writes_output = false;
    mutation[1].writes_output = true;
    assert!(matches!(
        reduction_rejects_without_mutation(&values, epoch, &mutation),
        ReductionProfileErrorV1::WrongOwner { actual: Some(1) }
    ));
}

#[test]
fn reduction_rejects_overflow_bad_output_and_output_substitution() {
    let values = [i32::MAX; 64];
    let trace = canonical_reduction_trace_v1(0);
    assert!(matches!(
        reduction_rejects_without_mutation(&values, 0, &trace),
        ReductionProfileErrorV1::SumOutOfRange { .. }
    ));

    let mut output = [10, 11];
    assert!(matches!(
        lds_reduction_oracle_v1(&[0; 64], 0, &trace, &mut output),
        Err(ReductionProfileErrorV1::InvalidOutputLength { provided: 2 })
    ));
    assert_eq!(output, [10, 11]);

    let mismatch = compare_reduction_output_v1(64, 63).unwrap_err();
    assert_eq!(mismatch.expected, 64);
    assert_eq!(mismatch.actual, 63);
}

#[test]
fn deterministic_atomic_vectors_compute_exact_eligible_sum() {
    for vector in atomic_vectors_v1() {
        let lanes = canonical_atomic_lanes_v1(&vector.values, &vector.eligible);
        let mut output = [u32::MAX];
        let expected = atomic_add_oracle_v1(
            vector.initial,
            canonical_atomic_profile_v1(),
            &lanes,
            &mut output,
        )
        .unwrap_or_else(|error| panic!("{} was rejected: {error:?}", vector.name));
        assert_eq!(expected, vector.expected, "{}", vector.name);
        assert_eq!(output, [vector.expected], "{}", vector.name);
        assert_eq!(compare_atomic_output_v1(expected, output[0]), Ok(()));
    }
}

#[test]
fn atomic_rejects_scope_order_address_lane_and_eligibility_mutations() {
    let values = [1_u32; 64];
    let eligible = [true; 64];
    let lanes = canonical_atomic_lanes_v1(&values, &eligible);

    let mut profile = canonical_atomic_profile_v1();
    profile.lane_count = 63;
    assert!(matches!(
        atomic_rejects_without_mutation(profile, &lanes),
        AtomicProfileErrorV1::InvalidLaneCount { provided: 63 }
    ));

    let mut profile = canonical_atomic_profile_v1();
    profile.address_space = AtomicAddressSpaceV1::Workgroup;
    assert!(matches!(
        atomic_rejects_without_mutation(profile, &lanes),
        AtomicProfileErrorV1::WrongAddressSpace { .. }
    ));

    let mut profile = canonical_atomic_profile_v1();
    profile.ordering = AtomicOrderingV1::AcquireRelease;
    assert!(matches!(
        atomic_rejects_without_mutation(profile, &lanes),
        AtomicProfileErrorV1::WrongOrdering { .. }
    ));

    let mut profile = canonical_atomic_profile_v1();
    profile.scope = AtomicScopeV1::Workgroup;
    assert!(matches!(
        atomic_rejects_without_mutation(profile, &lanes),
        AtomicProfileErrorV1::WrongScope { .. }
    ));

    let mut profile = canonical_atomic_profile_v1();
    profile.target_index = 1;
    assert!(matches!(
        atomic_rejects_without_mutation(profile, &lanes),
        AtomicProfileErrorV1::WrongTargetIndex { actual: 1 }
    ));

    assert!(matches!(
        atomic_rejects_without_mutation(canonical_atomic_profile_v1(), &lanes[..63]),
        AtomicProfileErrorV1::InvalidLaneVectorLength { provided: 63 }
    ));

    let mut mutation = lanes.clone();
    mutation[63].lane = 64;
    assert!(matches!(
        atomic_rejects_without_mutation(canonical_atomic_profile_v1(), &mutation),
        AtomicProfileErrorV1::LaneOutOfRange { lane: 64 }
    ));

    let mut mutation = lanes;
    mutation[63].lane = 62;
    assert!(matches!(
        atomic_rejects_without_mutation(canonical_atomic_profile_v1(), &mutation),
        AtomicProfileErrorV1::DuplicateLane { lane: 62 }
    ));

    let ineligible = canonical_atomic_lanes_v1(&[u32::MAX; 64], &[false; 64]);
    let mut output = [0];
    assert_eq!(
        atomic_add_oracle_v1(9, canonical_atomic_profile_v1(), &ineligible, &mut output),
        Ok(9)
    );
    assert_eq!(output, [9], "ineligible values must not contribute");
}

#[test]
fn atomic_rejects_overflow_bad_output_and_output_substitution() {
    let lanes = canonical_atomic_lanes_v1(&[u32::MAX; 64], &[true; 64]);
    assert!(matches!(
        atomic_rejects_without_mutation(canonical_atomic_profile_v1(), &lanes),
        AtomicProfileErrorV1::SumOutOfRange { .. }
    ));

    let lanes = canonical_atomic_lanes_v1(&[0; 64], &[false; 64]);
    let mut output = [10, 11];
    assert!(matches!(
        atomic_add_oracle_v1(0, canonical_atomic_profile_v1(), &lanes, &mut output),
        Err(AtomicProfileErrorV1::InvalidOutputLength { provided: 2 })
    ));
    assert_eq!(output, [10, 11]);

    let mismatch = compare_atomic_output_v1(9, 10).unwrap_err();
    assert_eq!(mismatch.expected, 9);
    assert_eq!(mismatch.actual, 10);
}

#[test]
fn fixed_seed_corpus_is_mode_independent() {
    let mut state = 0x9e37_79b9_u32;
    for epoch in 0..128_u32 {
        let mut reduction = [0_i32; 64];
        let mut values = [0_u32; 64];
        let mut eligible = [false; 64];
        for lane in 0..64 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            reduction[lane] = ((state >> 8) % 2_001) as i32 - 1_000;
            values[lane] = (state >> 16) & 0xff;
            eligible[lane] = state & 3 != 0;
        }

        let trace = canonical_reduction_trace_v1(epoch);
        let expected_reduction = reduction.iter().copied().sum::<i32>();
        let mut reduction_output = [0];
        assert_eq!(
            lds_reduction_oracle_v1(&reduction, epoch, &trace, &mut reduction_output),
            Ok(expected_reduction)
        );

        let lanes = canonical_atomic_lanes_v1(&values, &eligible);
        let expected_atomic = values
            .iter()
            .zip(eligible)
            .filter_map(|(&value, active)| active.then_some(value))
            .sum::<u32>();
        let mut atomic_output = [0];
        assert_eq!(
            atomic_add_oracle_v1(0, canonical_atomic_profile_v1(), &lanes, &mut atomic_output),
            Ok(expected_atomic)
        );
    }
}
