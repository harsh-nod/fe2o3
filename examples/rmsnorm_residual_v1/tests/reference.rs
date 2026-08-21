use fe2o3_rmsnorm_residual_v1::{
    B3RmsNormBucketV1, Bf16V1, Qwen3ModelRoleV1, RmsNormArithmeticStageV1, RmsNormBufferV1,
    RmsNormProfileDescriptorV1, RmsNormReferenceErrorV1, rmsnorm_residual_f64_oracle_v1,
    rmsnorm_residual_reference_v1, validate_rmsnorm_profile_v1,
};

fn bf16(value: f32) -> Bf16V1 {
    Bf16V1::from_f32_rne(value).unwrap()
}

fn profile(
    role: Qwen3ModelRoleV1,
    bucket: B3RmsNormBucketV1,
) -> fe2o3_rmsnorm_residual_v1::ValidatedRmsNormProfileV1 {
    validate_rmsnorm_profile_v1(RmsNormProfileDescriptorV1::canonical(role, bucket)).unwrap()
}

#[test]
fn zero_and_constant_rows_follow_the_declared_equations() {
    let profile = profile(Qwen3ModelRoleV1::Draft06B, B3RmsNormBucketV1::DecodeS1);
    let elements = profile.resources().activation_elements;
    let hidden = profile.descriptor().hidden_size;

    let zero = vec![bf16(0.0); elements];
    let one = vec![bf16(1.0); elements];
    let weight = vec![bf16(1.0); hidden];
    let mut normalized = vec![bf16(-9.0); elements];
    let mut residual_output = vec![bf16(-9.0); elements];
    let state = rmsnorm_residual_reference_v1(
        profile,
        &zero,
        &one,
        &weight,
        &mut normalized,
        &mut residual_output,
    )
    .unwrap();

    assert_eq!(state.rows, 1);
    assert!(state.minimum_reciprocal_rms.is_finite());
    assert_eq!(state.minimum_reciprocal_rms, state.maximum_reciprocal_rms);
    assert!(normalized.iter().all(|value| *value == bf16(1.0)));
    assert!(residual_output.iter().all(|value| *value == bf16(1.0)));

    let mut normalized = vec![bf16(-9.0); elements];
    let mut residual_output = vec![bf16(-9.0); elements];
    rmsnorm_residual_reference_v1(
        profile,
        &zero,
        &zero,
        &weight,
        &mut normalized,
        &mut residual_output,
    )
    .unwrap();
    assert!(normalized.iter().all(|value| value.to_bits() == 0));
    assert!(residual_output.iter().all(|value| value.to_bits() == 0));
}

fn deterministic_inputs(length: usize, seed: u32) -> Vec<Bf16V1> {
    let mut state = seed;
    (0..length)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let signed = ((state >> 8) % 2_001) as i32 - 1_000;
            bf16(signed as f32 / 512.0)
        })
        .collect()
}

#[test]
fn schedule_model_agrees_with_independent_f64_oracle_for_target_and_draft() {
    let cases = [
        (Qwen3ModelRoleV1::Target8B, B3RmsNormBucketV1::DecodeS1),
        (
            Qwen3ModelRoleV1::Target8B,
            B3RmsNormBucketV1::SpeculativeS1K4,
        ),
        (Qwen3ModelRoleV1::Draft06B, B3RmsNormBucketV1::DecodeS8),
        (
            Qwen3ModelRoleV1::Draft06B,
            B3RmsNormBucketV1::SpeculativeS1K16,
        ),
    ];

    for (case_index, (role, bucket)) in cases.into_iter().enumerate() {
        let profile = profile(role, bucket);
        let elements = profile.resources().activation_elements;
        let hidden = profile.descriptor().hidden_size;
        let activation = deterministic_inputs(elements, 17 + case_index as u32);
        let residual = deterministic_inputs(elements, 91 + case_index as u32);
        let weight: Vec<_> = deterministic_inputs(hidden, 301 + case_index as u32)
            .into_iter()
            .map(|value| bf16(1.0 + value.to_f32() * 0.125))
            .collect();
        let oracle =
            rmsnorm_residual_f64_oracle_v1(profile, &activation, &residual, &weight).unwrap();
        let mut normalized = vec![Bf16V1::default(); elements];
        let mut residual_output = vec![Bf16V1::default(); elements];
        rmsnorm_residual_reference_v1(
            profile,
            &activation,
            &residual,
            &weight,
            &mut normalized,
            &mut residual_output,
        )
        .unwrap();

        for index in 0..elements {
            let actual_residual = f64::from(residual_output[index].to_f32());
            let residual_allowance = 0.008_f64.max(oracle.residual_sum[index].abs() * 0.008);
            assert!(
                (actual_residual - oracle.residual_sum[index]).abs() <= residual_allowance,
                "residual mismatch at {role:?}/{bucket:?}/{index}"
            );

            let actual_normalized = f64::from(normalized[index].to_f32());
            let normalized_allowance = 0.02_f64.max(oracle.normalized[index].abs() * 0.02);
            assert!(
                (actual_normalized - oracle.normalized[index]).abs() <= normalized_allowance,
                "normalized mismatch at {role:?}/{bucket:?}/{index}: expected {}, actual {}",
                oracle.normalized[index],
                actual_normalized
            );
        }
    }
}

#[test]
fn preflight_and_arithmetic_failures_leave_both_outputs_unchanged() {
    let profile = profile(Qwen3ModelRoleV1::Draft06B, B3RmsNormBucketV1::DecodeS1);
    let elements = profile.resources().activation_elements;
    let hidden = profile.descriptor().hidden_size;
    let activation = vec![bf16(1.0); elements];
    let residual = vec![bf16(0.0); elements];
    let weight = vec![bf16(1.0); hidden];
    let sentinel = vec![Bf16V1::from_bits(0x4242); elements];

    let mut normalized = sentinel.clone();
    let mut residual_output = sentinel.clone();
    assert_eq!(
        rmsnorm_residual_reference_v1(
            profile,
            &activation[..elements - 1],
            &residual,
            &weight,
            &mut normalized,
            &mut residual_output,
        ),
        Err(RmsNormReferenceErrorV1::WrongLength {
            buffer: RmsNormBufferV1::Activation,
            expected: elements,
            actual: elements - 1,
        })
    );
    assert_eq!(normalized, sentinel);
    assert_eq!(residual_output, sentinel);

    let mut hostile = activation.clone();
    hostile[hidden / 2] = Bf16V1::from_bits(0x7f80);
    let mut normalized = sentinel.clone();
    let mut residual_output = sentinel.clone();
    assert_eq!(
        rmsnorm_residual_reference_v1(
            profile,
            &hostile,
            &residual,
            &weight,
            &mut normalized,
            &mut residual_output,
        ),
        Err(RmsNormReferenceErrorV1::NonFiniteInput {
            buffer: RmsNormBufferV1::Activation,
            index: hidden / 2,
        })
    );
    assert_eq!(normalized, sentinel);
    assert_eq!(residual_output, sentinel);

    let huge = vec![Bf16V1::from_bits(0x7f7f); elements];
    let mut normalized = sentinel.clone();
    let mut residual_output = sentinel.clone();
    assert_eq!(
        rmsnorm_residual_reference_v1(
            profile,
            &huge,
            &huge,
            &weight,
            &mut normalized,
            &mut residual_output,
        ),
        Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
            row: 0,
            stage: RmsNormArithmeticStageV1::ResidualAdd,
        })
    );
    assert_eq!(normalized, sentinel);
    assert_eq!(residual_output, sentinel);
}

#[test]
fn bf16_rounding_is_ties_to_even_and_rejects_nonfinite_values() {
    assert_eq!(Bf16V1::from_f32_rne(1.0).unwrap().to_bits(), 0x3f80);
    assert_eq!(Bf16V1::from_f32_rne(-0.0).unwrap().to_bits(), 0x8000);
    assert_eq!(
        Bf16V1::from_f32_rne(f32::from_bits(0x3f80_8000))
            .unwrap()
            .to_bits(),
        0x3f80
    );
    assert_eq!(
        Bf16V1::from_f32_rne(f32::from_bits(0x3f81_8000))
            .unwrap()
            .to_bits(),
        0x3f82
    );
    assert!(Bf16V1::from_f32_rne(f32::INFINITY).is_err());
    assert!(Bf16V1::from_f32_rne(f32::NAN).is_err());
}
