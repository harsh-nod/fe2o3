use fe2o3_wave64_collectives_v1::{
    CollectiveOutputV1, OracleErrorV1OrMismatch, OutputMismatchV1, compare_wave64_collectives_v1,
    wave64_collectives_oracle_v1,
};

fn evidence() -> ([f32; 64], u64, [f32; 64], [f32; 64], [f32; 64]) {
    let input = core::array::from_fn(|lane| (lane + 1) as f32);
    let mask =
        (1_u64 << 0) | (1_u64 << 2) | (1_u64 << 5) | (1_u64 << 17) | (1_u64 << 41) | (1_u64 << 63);
    let mut reduction = [0.0; 64];
    let mut inclusive = [0.0; 64];
    let mut exclusive = [0.0; 64];
    wave64_collectives_oracle_v1(&input, mask, &mut reduction, &mut inclusive, &mut exclusive)
        .unwrap();
    (input, mask, reduction, inclusive, exclusive)
}

fn assert_mismatch(
    input: &[f32],
    mask: u64,
    reduction: &[f32],
    inclusive: &[f32],
    exclusive: &[f32],
    output: CollectiveOutputV1,
) -> OutputMismatchV1 {
    match compare_wave64_collectives_v1(input, mask, reduction, inclusive, exclusive) {
        Err(OracleErrorV1OrMismatch::Mismatch(mismatch)) => {
            assert_eq!(mismatch.output, output);
            mismatch
        }
        result => panic!("mutation was not rejected as an exact mismatch: {result:?}"),
    }
}

#[test]
fn wrong_mask_is_detected() {
    let (input, mask, reduction, inclusive, exclusive) = evidence();
    assert_mismatch(
        &input,
        mask ^ (1_u64 << 9),
        &reduction,
        &inclusive,
        &exclusive,
        CollectiveOutputV1::Reduction,
    );
}

#[test]
fn wrong_width_is_rejected_before_comparison() {
    let (input, mask, reduction, inclusive, exclusive) = evidence();
    assert!(matches!(
        compare_wave64_collectives_v1(&input[..63], mask, &reduction, &inclusive, &exclusive),
        Err(OracleErrorV1OrMismatch::Admission(_))
    ));
}

#[test]
fn wrong_lane_ownership_is_detected() {
    let (input, mask, reduction, mut inclusive, exclusive) = evidence();
    inclusive.swap(5, 17);
    let mismatch = assert_mismatch(
        &input,
        mask,
        &reduction,
        &inclusive,
        &exclusive,
        CollectiveOutputV1::Inclusive,
    );
    assert!(matches!(mismatch.lane, 5 | 17));
}

#[test]
fn wrong_reduction_is_detected() {
    let (input, mask, mut reduction, inclusive, exclusive) = evidence();
    reduction[41] += 1.0;
    let mismatch = assert_mismatch(
        &input,
        mask,
        &reduction,
        &inclusive,
        &exclusive,
        CollectiveOutputV1::Reduction,
    );
    assert_eq!(mismatch.lane, 41);
}

#[test]
fn inclusive_exclusive_ordering_swap_is_detected() {
    let (input, mask, reduction, inclusive, exclusive) = evidence();
    assert_mismatch(
        &input,
        mask,
        &reduction,
        &exclusive,
        &inclusive,
        CollectiveOutputV1::Inclusive,
    );
}

#[test]
fn cross_output_substitution_is_detected() {
    let (input, mask, reduction, inclusive, exclusive) = evidence();
    assert_mismatch(
        &input,
        mask,
        &inclusive,
        &inclusive,
        &exclusive,
        CollectiveOutputV1::Reduction,
    );
    assert_mismatch(
        &input,
        mask,
        &reduction,
        &reduction,
        &exclusive,
        CollectiveOutputV1::Inclusive,
    );
}
