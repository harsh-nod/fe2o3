use fe2o3_row_softmax_v1::{
    EMPTY_ROW_POLICY_V1, MASK_POLICY_V1, ROW_BYTES_V1, ROW_ELEMENTS_V1, ReferenceErrorV1,
    lane_access_v1, row_softmax_reference_v1,
};

#[test]
fn fixed_contract_has_one_nonempty_unmasked_row() {
    assert_eq!(ROW_ELEMENTS_V1, 64);
    assert_eq!(ROW_BYTES_V1, 256);
    assert_eq!(MASK_POLICY_V1, "unmasked: all 64 positions participate");
    assert!(EMPTY_ROW_POLICY_V1.contains("not representable"));
}

#[test]
fn lane_ownership_is_bounded_and_injective() {
    for lane in 0..ROW_ELEMENTS_V1 {
        let access = lane_access_v1(lane).unwrap();
        assert_eq!(access.input_index, lane);
        assert_eq!(access.scratch_index, lane);
        assert_eq!(access.output_index, lane);
        for other in 0..ROW_ELEMENTS_V1 {
            if lane != other {
                assert_ne!(
                    access.output_index,
                    lane_access_v1(other).unwrap().output_index
                );
                assert_ne!(
                    access.scratch_index,
                    lane_access_v1(other).unwrap().scratch_index
                );
            }
        }
    }
    assert_eq!(lane_access_v1(ROW_ELEMENTS_V1), None);
    assert_eq!(lane_access_v1(usize::MAX), None);
}

#[test]
fn equal_values_produce_uniform_output() {
    let input = [7.25_f32; ROW_ELEMENTS_V1];
    let mut output = [f32::NAN; ROW_ELEMENTS_V1];
    let state = row_softmax_reference_v1(&input, &mut output).unwrap();

    assert_eq!(state.maximum, 7.25);
    assert_eq!(state.weights, [1.0; ROW_ELEMENTS_V1]);
    assert_eq!(state.denominator, 64.0);
    assert_eq!(output, [1.0 / 64.0; ROW_ELEMENTS_V1]);
}

#[test]
fn stable_shift_handles_large_finite_values() {
    let mut input = [10_000.0_f32; ROW_ELEMENTS_V1];
    input[13] = 10_001.0;
    let mut output = [0.0; ROW_ELEMENTS_V1];
    let state = row_softmax_reference_v1(&input, &mut output).unwrap();

    assert_eq!(state.maximum, 10_001.0);
    assert!(
        state
            .weights
            .iter()
            .all(|weight| *weight > 0.0 && *weight <= 1.0)
    );
    assert!(output[13] > output[12]);
    let sum: f32 = output.iter().copied().sum();
    assert!((sum - 1.0).abs() <= 8.0 * f32::EPSILON);
}

#[test]
fn nonfinite_input_is_rejected_before_output_mutation() {
    for (index, invalid) in [(0, f32::NAN), (17, f32::INFINITY), (63, f32::NEG_INFINITY)] {
        let mut input = [0.0_f32; ROW_ELEMENTS_V1];
        input[index] = invalid;
        let mut output = [19.0_f32; ROW_ELEMENTS_V1];
        assert_eq!(
            row_softmax_reference_v1(&input, &mut output),
            Err(ReferenceErrorV1::NonFiniteInput { index })
        );
        assert_eq!(output, [19.0; ROW_ELEMENTS_V1]);
    }
}
