use fe2o3_row_softmax_v1::{
    GFX942_OCML_COMPARISON_POLICY_V1, HOST_ORACLE_EXPONENTIAL_V1, MAX_ROW_ELEMENTS_V1,
    SoftmaxComparisonErrorV1, SoftmaxComparisonPolicyV1, SoftmaxContractErrorV1,
    SoftmaxExponentialV1, compare_row_softmax_v1, row_softmax_oracle_v1,
};

#[test]
fn rejects_invalid_shapes_inputs_and_masks_without_mutating_output() {
    let mut output = [19.0_f32; 4];
    assert_eq!(
        row_softmax_oracle_v1(&[], None, &mut []),
        Err(SoftmaxContractErrorV1::EmptyRow)
    );
    assert_eq!(
        row_softmax_oracle_v1(&[0.0; 3], None, &mut output),
        Err(SoftmaxContractErrorV1::LengthMismatch)
    );
    assert_eq!(
        row_softmax_oracle_v1(&[0.0; 4], Some(&[true; 3]), &mut output),
        Err(SoftmaxContractErrorV1::LengthMismatch)
    );
    assert_eq!(
        row_softmax_oracle_v1(&[0.0; 4], Some(&[false; 4]), &mut output),
        Err(SoftmaxContractErrorV1::NoActiveElements)
    );
    for (index, invalid) in [(0, f32::NAN), (1, f32::INFINITY), (3, f32::NEG_INFINITY)] {
        let mut input = [0.0; 4];
        input[index] = invalid;
        assert_eq!(
            row_softmax_oracle_v1(&input, Some(&[true, false, true, true]), &mut output),
            Err(SoftmaxContractErrorV1::NonFiniteInput { index })
        );
    }
    assert_eq!(output, [19.0; 4]);

    let oversized = vec![0.0; MAX_ROW_ELEMENTS_V1 + 1];
    let mut oversized_output = vec![7.0; oversized.len()];
    assert_eq!(
        row_softmax_oracle_v1(&oversized, None, &mut oversized_output),
        Err(SoftmaxContractErrorV1::RowTooLarge)
    );
    assert!(oversized_output.iter().all(|value| *value == 7.0));
}

#[test]
fn mask_signed_zero_subnormal_and_underflow_policies_are_explicit() {
    let input = [-0.0, 0.0, f32::from_bits(1), -200.0];
    let mask = [true, false, true, true];
    let mut output = [f32::NAN; 4];
    let state = row_softmax_oracle_v1(&input, Some(&mask), &mut output).unwrap();

    assert_eq!(state.maximum.to_bits(), f32::from_bits(1).to_bits());
    assert_eq!(state.active_elements, 3);
    assert_eq!(output[1].to_bits(), 0.0_f32.to_bits());
    assert_eq!(output[3], 0.0);
    assert!(output[0] > 0.0 && output[2] > 0.0);
    compare_row_softmax_v1(
        &output,
        &output,
        Some(&mask),
        GFX942_OCML_COMPARISON_POLICY_V1,
    )
    .unwrap();
}

#[test]
fn deterministic_corpus_covers_equal_dominant_cancellation_and_random_rows() {
    let mut rows = vec![
        vec![0.0; 64],
        (0..64).map(|index| index as f32 - 32.0).collect(),
        (0..64)
            .map(|index| if index == 17 { 10_001.0 } else { 10_000.0 })
            .collect(),
        (0..64)
            .map(|index| if index % 2 == 0 { 8.0 } else { -8.0 })
            .collect(),
    ];
    let mut state = 0x6d2b_79f5_u32;
    rows.push(
        (0..64)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((state >> 8) as i32 % 4096) as f32 / 256.0
            })
            .collect(),
    );

    for input in rows {
        let mut output = vec![f32::NAN; input.len()];
        let observed = row_softmax_oracle_v1(&input, None, &mut output).unwrap();
        assert!(observed.maximum.is_finite());
        assert!(observed.denominator >= 1.0);
        compare_row_softmax_v1(&output, &output, None, GFX942_OCML_COMPARISON_POLICY_V1).unwrap();
    }

    assert_eq!(
        HOST_ORACLE_EXPONENTIAL_V1,
        SoftmaxExponentialV1::RustStdF64Exp
    );
    assert_eq!(
        GFX942_OCML_COMPARISON_POLICY_V1.device_exponential,
        SoftmaxExponentialV1::OcmlExpF32
    );

    let mut constant = [f32::NAN; 64];
    row_softmax_oracle_v1(&[0.0; 64], None, &mut constant).unwrap();
    assert!(
        constant
            .iter()
            .all(|value| value.to_bits() == (1.0_f32 / 64.0).to_bits())
    );
}

#[test]
fn comparison_rejects_policy_expected_output_and_mask_substitutions() {
    let input = [0.0, 1.0, 2.0, 3.0];
    let mask = [true, false, true, true];
    let mut expected = [0.0; 4];
    row_softmax_oracle_v1(&input, Some(&mask), &mut expected).unwrap();

    let mut wrong = expected;
    wrong[2] += 0.01;
    assert!(matches!(
        compare_row_softmax_v1(
            &expected,
            &wrong,
            Some(&mask),
            GFX942_OCML_COMPARISON_POLICY_V1
        ),
        Err(SoftmaxComparisonErrorV1::OutputMismatch { index: 2, .. })
    ));

    let mut substituted_expected = expected;
    substituted_expected[0] += 0.01;
    assert!(matches!(
        compare_row_softmax_v1(
            &substituted_expected,
            &expected,
            Some(&mask),
            GFX942_OCML_COMPARISON_POLICY_V1
        ),
        Err(SoftmaxComparisonErrorV1::OutputMismatch { index: 0, .. })
    ));

    let mut wrong_masked = expected;
    wrong_masked[1] = -0.0;
    assert_eq!(
        compare_row_softmax_v1(
            &expected,
            &wrong_masked,
            Some(&mask),
            GFX942_OCML_COMPARISON_POLICY_V1
        ),
        Err(SoftmaxComparisonErrorV1::MaskedOutputNotPositiveZero { index: 1 })
    );

    let invalid_policy = SoftmaxComparisonPolicyV1 {
        absolute_tolerance: f32::NAN,
        ..GFX942_OCML_COMPARISON_POLICY_V1
    };
    assert_eq!(
        compare_row_softmax_v1(&expected, &expected, Some(&mask), invalid_policy),
        Err(SoftmaxComparisonErrorV1::InvalidPolicy)
    );

    let mut non_finite = expected;
    non_finite[0] = f32::NAN;
    assert_eq!(
        compare_row_softmax_v1(
            &expected,
            &non_finite,
            Some(&mask),
            GFX942_OCML_COMPARISON_POLICY_V1
        ),
        Err(SoftmaxComparisonErrorV1::InvalidOutput { index: 0 })
    );

    let mut wrong_sum = expected;
    for value in &mut wrong_sum {
        *value *= 0.99;
    }
    wrong_sum[1] = 0.0;
    let sum_only_policy = SoftmaxComparisonPolicyV1 {
        absolute_tolerance: 1.0,
        relative_tolerance: 1.0,
        maximum_ulps: u32::MAX,
        ..GFX942_OCML_COMPARISON_POLICY_V1
    };
    assert!(matches!(
        compare_row_softmax_v1(&expected, &wrong_sum, Some(&mask), sum_only_policy),
        Err(SoftmaxComparisonErrorV1::SumMismatch { .. })
    ));
}
