use fe2o3_gfx950_advanced_attention::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, DEEPSEEK_INVALID_TOKEN_V1, HEAD_DIMENSION_V1, KDA_TAPS_V1,
    MIXING_STREAMS_V1, PREFILL_TOKENS_V1,
    reference::{
        ReferenceErrorV1, attnres_aggregate_reference_v1, compressed_hybrid_attention_reference_v1,
        content_sparse_attention_reference_v1, deepseek_sparse_attention_reference_v1,
        four_branch_residual_reference_v1, kda_gdn_decode_reference_v1,
        kda_gdn_prefill_reference_v1, mhc_sinkhorn_matrix_reference_v1,
        mhc_sinkhorn_mix_reference_v1,
    },
};

fn deterministic_floats(count: usize, salt: usize, scale: f32) -> Vec<f32> {
    (0..count)
        .map(|index| {
            let centered = ((index * 17 + salt * 11) % 19) as i32 - 9;
            scale * centered as f32 / 9.0
        })
        .collect()
}

fn deterministic_fp8(count: usize, salt: usize) -> Vec<u8> {
    let values = [0xb8_u8, 0xb0, 0x00, 0x30, 0x38];
    (0..count)
        .map(|index| values[(index * 3 + salt) % values.len()])
        .collect()
}

fn assert_finite(values: &[f32]) {
    assert!(values.iter().all(|value| value.is_finite()));
}

#[test]
fn kda_decode_and_prefill_cover_the_exact_recurrence_shapes() {
    let weights = [0.5_f32, -0.25, 0.125];
    let initial = deterministic_floats(CHANNELS_V1, 3, 0.5);
    let decode = kda_gdn_decode_reference_v1(
        &deterministic_floats(KDA_TAPS_V1 * CHANNELS_V1, 1, 0.6),
        &deterministic_floats(CHANNELS_V1, 2, 0.8),
        &initial,
        &weights,
    )
    .unwrap();
    assert_eq!(decode.state.len(), CHANNELS_V1);
    assert_eq!(decode.normalized.len(), CHANNELS_V1);
    assert_finite(&decode.state);
    assert_finite(&decode.normalized);

    let prefill = kda_gdn_prefill_reference_v1(
        &deterministic_floats(PREFILL_TOKENS_V1 * CHANNELS_V1, 4, 0.5),
        &deterministic_floats(PREFILL_TOKENS_V1 * CHANNELS_V1, 5, 0.7),
        &initial,
        &weights,
    )
    .unwrap();
    assert_eq!(prefill.final_state.len(), CHANNELS_V1);
    assert_eq!(prefill.normalized.len(), PREFILL_TOKENS_V1 * CHANNELS_V1);
    assert_finite(&prefill.final_state);
    assert_finite(&prefill.normalized);
    assert_ne!(prefill.final_state, decode.state);
}

#[test]
fn indexed_sparse_attention_selects_the_fixture_ids() {
    let q = deterministic_fp8(HEAD_DIMENSION_V1, 1);
    let k = deterministic_fp8(ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1, 2);
    let v = deterministic_fp8(ATTENTION_TOKENS_V1 * CHANNELS_V1, 3);
    let content_scores = [
        0.10_f32, 0.82, -0.20, 0.35, 0.61, 0.55, 0.14, 0.92, 0.73, -0.10, 0.48, 0.31, 0.41, 0.67,
        0.22, 0.05,
    ];
    let result = content_sparse_attention_reference_v1(&q, &k, &v, &content_scores).unwrap();
    assert_eq!(result.selected, [7, 1, 4]);
    assert_eq!(result.output.len(), CHANNELS_V1);
    assert_finite(&result.output);
}

#[test]
fn sparse_selection_uses_numeric_signed_zero_ties_and_lower_ids() {
    let q = vec![0; HEAD_DIMENSION_V1];
    let k = vec![0; ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1];
    let v = vec![0; ATTENTION_TOKENS_V1 * CHANNELS_V1];
    let mut content_scores = [-1.0_f32; ATTENTION_TOKENS_V1];
    content_scores[0] = -0.0;
    content_scores[1] = 0.0;
    content_scores[4] = -0.0;

    let result = content_sparse_attention_reference_v1(&q, &k, &v, &content_scores).unwrap();
    assert_eq!(result.selected, [0, 1, 4]);
}

#[test]
fn deepseek_sparse_attention_consumes_only_valid_indexer_tokens() {
    let q = deterministic_floats(HEAD_DIMENSION_V1, 1, 0.5);
    let k = deterministic_floats(ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1, 2, 0.5);
    let v = deterministic_floats(ATTENTION_TOKENS_V1 * CHANNELS_V1, 3, 0.5);
    let result =
        deepseek_sparse_attention_reference_v1(&q, &k, &v, &[13, DEEPSEEK_INVALID_TOKEN_V1, 2, 9])
            .unwrap();
    assert_eq!(result.output.len(), CHANNELS_V1);
    assert_finite(&result.output);
    assert!(result.softmax_maximum.is_finite());
    assert!(result.softmax_normalizer.is_finite());
    assert!((1.0..=3.0).contains(&result.softmax_normalizer));
}

#[test]
fn deepseek_sparse_attention_rejects_duplicate_or_empty_valid_domains() {
    let q = deterministic_floats(HEAD_DIMENSION_V1, 1, 0.5);
    let k = deterministic_floats(ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1, 2, 0.5);
    let v = deterministic_floats(ATTENTION_TOKENS_V1 * CHANNELS_V1, 3, 0.5);
    assert_eq!(
        deepseek_sparse_attention_reference_v1(&q, &k, &v, &[2, 7, 2, 9]),
        Err(ReferenceErrorV1::InvalidSparseIndices),
    );
    assert_eq!(
        deepseek_sparse_attention_reference_v1(&q, &k, &v, &[DEEPSEEK_INVALID_TOKEN_V1; 4],),
        Err(ReferenceErrorV1::InvalidSparseIndices),
    );
}

#[test]
fn compressed_hybrid_attention_returns_one_finite_channel_tile() {
    let result = compressed_hybrid_attention_reference_v1(
        &deterministic_fp8(HEAD_DIMENSION_V1, 1),
        &deterministic_fp8(ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1, 2),
        &deterministic_fp8(ATTENTION_TOKENS_V1 * CHANNELS_V1, 3),
        &deterministic_floats(ATTENTION_TOKENS_V1, 7, 0.4),
    )
    .unwrap();
    assert_eq!(result.len(), CHANNELS_V1);
    assert_finite(&result);
    assert!(result.iter().any(|value| *value != 0.0));
}

#[test]
fn attnres_and_four_branch_residual_follow_bounded_mixing_policies() {
    let depth_values = deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 8, 0.7);
    let attnres = attnres_aggregate_reference_v1(
        &depth_values,
        &deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 9, 0.9),
    )
    .unwrap();
    for channel in 0..CHANNELS_V1 {
        let values = (0..MIXING_STREAMS_V1)
            .map(|depth| depth_values[depth * CHANNELS_V1 + channel])
            .collect::<Vec<_>>();
        let minimum = values.iter().copied().fold(f32::INFINITY, f32::min);
        let maximum = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!((minimum..=maximum).contains(&attnres[channel]));
    }

    let residual = deterministic_floats(CHANNELS_V1, 10, 0.4);
    let branches = vec![1.0_f32; MIXING_STREAMS_V1 * CHANNELS_V1];
    let output = four_branch_residual_reference_v1(
        &residual,
        &branches,
        &vec![0.0; MIXING_STREAMS_V1 * CHANNELS_V1],
    )
    .unwrap();
    for channel in 0..CHANNELS_V1 {
        assert!((output[channel] - (residual[channel] + 0.5)).abs() < 1.0e-6);
    }
}

#[test]
fn mhc_sinkhorn_matrix_and_stream_mix_are_finite_and_bounded() {
    let logits = deterministic_floats(MIXING_STREAMS_V1 * MIXING_STREAMS_V1, 14, 0.5);
    let matrix = mhc_sinkhorn_matrix_reference_v1(&logits).unwrap();
    for column in 0..MIXING_STREAMS_V1 {
        let sum = (0..MIXING_STREAMS_V1)
            .map(|row| matrix[row * MIXING_STREAMS_V1 + column])
            .sum::<f32>();
        assert!((sum - 1.0).abs() < 1.0e-5);
    }
    let streams = deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 13, 0.7);
    let output = mhc_sinkhorn_mix_reference_v1(&streams, &logits).unwrap();
    assert_eq!(output.len(), MIXING_STREAMS_V1 * CHANNELS_V1);
    assert_finite(&output);
}

#[test]
fn references_reject_wrong_shapes_and_non_finite_inputs() {
    assert_eq!(
        kda_gdn_decode_reference_v1(&[], &[], &[], &[]),
        Err(ReferenceErrorV1::Shape)
    );
    let mut logits = vec![0.0_f32; MIXING_STREAMS_V1 * MIXING_STREAMS_V1];
    logits[0] = f32::INFINITY;
    assert_eq!(
        mhc_sinkhorn_matrix_reference_v1(&logits),
        Err(ReferenceErrorV1::NonFinite)
    );
}
