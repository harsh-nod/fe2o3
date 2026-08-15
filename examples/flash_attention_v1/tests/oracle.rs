use fe2o3_flash_attention_v1::{
    ArithmeticStageV1, FlashAttentionOracleErrorV1, TensorV1, deterministic_vectors_v1,
    flash_attention_oracle_v1,
};

const ELEMENTS: usize = 128;

fn vector(name: &str) -> fe2o3_flash_attention_v1::DeterministicVectorV1 {
    deterministic_vectors_v1()
        .into_iter()
        .find(|vector| vector.name == name)
        .unwrap()
}

#[test]
fn complete_deterministic_corpus_is_finite_and_stable() {
    let expected_names = [
        "nominal-mixed-sign",
        "all-equal-logits",
        "latest-causal-key-dominates",
        "causal-mask-weight-probe",
    ];
    let vectors = deterministic_vectors_v1();
    assert_eq!(
        std::array::from_fn(|index| vectors[index].name),
        expected_names
    );

    for vector in vectors {
        let mut output = [f32::NAN; ELEMENTS];
        let state = flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut output)
            .expect(vector.name);
        assert!(output.iter().all(|value| value.is_finite()));
        assert!(state.row_maxima.iter().all(|value| value.is_finite()));
        assert!(
            state
                .row_denominators
                .iter()
                .all(|value| value.is_finite() && *value >= 1.0 && *value <= 8.0)
        );
    }
}

#[test]
fn all_equal_logits_produce_exact_causal_prefix_means() {
    let vector = vector("all-equal-logits");
    let mut output = [0.0; ELEMENTS];
    flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut output).unwrap();

    for query_row in 0..8 {
        for column in 0..16 {
            let expected = (0..=query_row)
                .map(|key_row| f64::from(vector.v[key_row * 16 + column]))
                .sum::<f64>()
                / (query_row + 1) as f64;
            assert_eq!(output[query_row * 16 + column], expected as f32);
        }
    }
}

#[test]
fn dominant_logits_select_the_latest_admitted_causal_value() {
    let vector = vector("latest-causal-key-dominates");
    let mut output = [0.0; ELEMENTS];
    flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut output).unwrap();

    for query_row in 0..8 {
        for column in 0..16 {
            let expected = vector.v[query_row * 16 + column];
            assert!((output[query_row * 16 + column] - expected).abs() <= 1.0e-5);
        }
    }
}

#[test]
fn future_keys_and_values_are_causally_masked() {
    let vector = vector("nominal-mixed-sign");
    let mut baseline = [0.0; ELEMENTS];
    flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut baseline).unwrap();

    for first_future_row in 1..8 {
        let mut mutated = vector.clone();
        for row in first_future_row..8 {
            for column in 0..16 {
                mutated.k[row * 16 + column] += 1000.0;
                mutated.v[row * 16 + column] += 1000.0;
            }
        }
        let mut observed = [0.0; ELEMENTS];
        flash_attention_oracle_v1(&mutated.q, &mutated.k, &mutated.v, &mut observed).unwrap();
        assert_eq!(
            &observed[..first_future_row * 16],
            &baseline[..first_future_row * 16]
        );
    }
}

#[test]
fn causal_weight_probe_has_zero_probability_for_future_keys() {
    let vector = vector("causal-mask-weight-probe");
    let mut output = [0.0; ELEMENTS];
    flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut output).unwrap();
    for query_row in 0..8 {
        let row = &output[query_row * 16..(query_row + 1) * 16];
        let probability_sum: f32 = row[..8].iter().sum();
        assert!((probability_sum - 1.0).abs() <= 2.0e-7);
        assert!(row[query_row + 1..8].iter().all(|value| *value == 0.0));
        assert!(row[8..].iter().all(|value| *value == 0.0));
    }
}

#[test]
fn wrong_extents_fail_closed_without_touching_output() {
    let vector = vector("nominal-mixed-sign");
    let sentinel = [123.5_f32; ELEMENTS];

    let mut output = sentinel;
    assert_eq!(
        flash_attention_oracle_v1(&vector.q[..127], &vector.k, &vector.v, &mut output),
        Err(FlashAttentionOracleErrorV1::WrongLength {
            tensor: TensorV1::Q,
            expected: ELEMENTS,
            actual: 127,
        })
    );
    assert_eq!(output, sentinel);

    let mut output = [123.5_f32; 127];
    assert_eq!(
        flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut output),
        Err(FlashAttentionOracleErrorV1::WrongLength {
            tensor: TensorV1::O,
            expected: ELEMENTS,
            actual: 127,
        })
    );
    assert_eq!(output, [123.5_f32; 127]);
}

#[test]
fn every_input_tensor_rejects_non_finite_values_without_output_writes() {
    let base = vector("nominal-mixed-sign");
    for (tensor, bits) in [
        (TensorV1::Q, f32::NAN.to_bits()),
        (TensorV1::K, f32::INFINITY.to_bits()),
        (TensorV1::V, f32::NEG_INFINITY.to_bits()),
    ] {
        let mut vector = base.clone();
        match tensor {
            TensorV1::Q => vector.q[37] = f32::from_bits(bits),
            TensorV1::K => vector.k[37] = f32::from_bits(bits),
            TensorV1::V => vector.v[37] = f32::from_bits(bits),
            TensorV1::O => unreachable!(),
        }
        let mut output = [91.0_f32; ELEMENTS];
        assert_eq!(
            flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut output),
            Err(FlashAttentionOracleErrorV1::NonFiniteInput { tensor, index: 37 })
        );
        assert_eq!(output, [91.0_f32; ELEMENTS]);
    }
}

#[test]
fn finite_inputs_with_overflowing_fp32_product_fail_closed() {
    let mut vector = vector("all-equal-logits");
    vector.q[0] = f32::MAX;
    vector.k[0] = 2.0;
    let mut output = [17.0_f32; ELEMENTS];
    assert_eq!(
        flash_attention_oracle_v1(&vector.q, &vector.k, &vector.v, &mut output),
        Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
            query_row: 0,
            key_row: 0,
            output_column: None,
            stage: ArithmeticStageV1::DotProduct,
        })
    );
    assert_eq!(output, [17.0_f32; ELEMENTS]);
}
