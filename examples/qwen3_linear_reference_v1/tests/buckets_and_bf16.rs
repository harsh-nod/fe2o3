use fe2o3_qwen3_linear_reference_v1::linear::*;

#[test]
fn every_b3_bucket_has_independent_sequence_token_context_and_row_counts() {
    use Qwen3LinearBucketV1 as B;
    use Qwen3LinearModeV1 as M;
    // Literal B3 contracts, including the target's extra speculative token.
    let cases = [
        (M::Prefill, B::PrefillS1T128, 1, 128, 128, 128, 128, 128),
        (M::Prefill, B::PrefillS8T128, 8, 128, 128, 128, 1024, 1024),
        (M::Prefill, B::PrefillS1T512, 1, 512, 512, 512, 512, 512),
        (
            M::Prefill,
            B::PrefillS1T2048,
            1,
            2048,
            2048,
            2048,
            2048,
            2048,
        ),
        (M::Decode, B::DecodeS1C8192, 1, 1, 1, 8192, 1, 1),
        (M::Decode, B::DecodeS8C8192, 8, 1, 1, 8192, 8, 8),
        (M::Decode, B::DecodeS32C8192, 32, 1, 1, 8192, 32, 32),
        (M::Speculative, B::SpeculativeS1K4C8192, 1, 5, 4, 8192, 5, 4),
        (
            M::Speculative,
            B::SpeculativeS8K4C8192,
            8,
            5,
            4,
            8192,
            40,
            32,
        ),
        (M::Speculative, B::SpeculativeS1K8C8192, 1, 9, 8, 8192, 9, 8),
        (
            M::Speculative,
            B::SpeculativeS1K16C8192,
            1,
            17,
            16,
            8192,
            17,
            16,
        ),
    ];
    for (mode, bucket, sequences, target_tokens, draft_tokens, context, target_m, draft_m) in cases
    {
        for (role, tokens, rows) in [
            (Qwen3LinearRoleV1::Target8B, target_tokens, target_m),
            (Qwen3LinearRoleV1::Draft06B, draft_tokens, draft_m),
        ] {
            let candidate = exact_qwen3_linear_candidate_v1(Qwen3LinearSelectionV1 {
                role,
                mode,
                bucket,
                operator: Qwen3B3OperatorV1::KeyProjection,
                layer: 0,
            })
            .unwrap();
            assert_eq!(
                candidate.dimensions.bucket,
                Qwen3LinearBucketDimensionsV1 {
                    sequences,
                    active_tokens: tokens,
                    context_tokens: context,
                }
            );
            assert_eq!(candidate.dimensions.m, rows);
        }
    }
}

#[test]
fn bf16_activation_exceptions_and_signed_subnormals_reach_the_shared_oracle() {
    let selected = Qwen3LinearSelectionV1 {
        role: Qwen3LinearRoleV1::Draft06B,
        mode: Qwen3LinearModeV1::Decode,
        bucket: Qwen3LinearBucketV1::DecodeS1C8192,
        operator: Qwen3B3OperatorV1::KeyProjection,
        layer: 0,
    };
    let candidate = exact_qwen3_linear_candidate_v1(selected).unwrap();
    let mut weight = vec![0; 1024 * 1024];
    weight[0] = 0x3f80;
    let initial = vec![0.0; 1024];
    for bits in [0x7fc0_u16, 0x7f80, 0xff80, 0x0001, 0x8001] {
        let mut activation = vec![0; 1024];
        activation[0] = bits;
        let output =
            execute_qwen3_linear_reference_v1(&candidate, selected, &activation, &weight, &initial)
                .unwrap();
        let widened = f32::from_bits(u32::from(bits) << 16);
        if widened.is_nan() {
            assert!(output[0].is_nan());
        } else {
            assert_eq!(output[0].to_bits(), widened.to_bits());
        }
        if widened.is_finite() {
            assert!(output[1..].iter().all(|x| *x == 0.0));
        } else {
            assert!(output[1..].iter().all(|x| x.is_nan()));
        }
    }
}
