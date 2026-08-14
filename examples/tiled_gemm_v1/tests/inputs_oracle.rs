use std::collections::BTreeSet;

use fe2o3_device::Bf16;
use fe2o3_tiled_gemm_v1::{
    ArithmeticOracleErrorV1, BF16_INPUT_PATTERN_V1, EvidenceInputErrorV1, EvidenceOperandV1,
    ShapeV1, generate_inputs_v1, tiled_gemm_arithmetic_oracle_v1, tiled_gemm_evidence_oracle_v1,
    validate_evidence_inputs_v1,
};

#[test]
fn generators_are_bitwise_deterministic_and_operand_separated() {
    let shape = ShapeV1::checked(16, 16, 16).unwrap();
    let first = generate_inputs_v1(shape, 0x1234_5678_9abc_def0);
    let second = generate_inputs_v1(shape, 0x1234_5678_9abc_def0);
    assert_eq!(first.a_bits(), second.a_bits());
    assert_eq!(first.b_bits(), second.b_bits());
    assert_ne!(first.a_bits(), first.b_bits());

    let changed_seed = generate_inputs_v1(shape, 0x1234_5678_9abc_def1);
    assert_ne!(first.a_bits(), changed_seed.a_bits());
    assert_ne!(first.b_bits(), changed_seed.b_bits());

    let changed_shape = generate_inputs_v1(ShapeV1::checked(16, 32, 16).unwrap(), 0x1234);
    let original_shape = generate_inputs_v1(shape, 0x1234);
    assert_ne!(original_shape.a_bits(), changed_shape.a_bits());
}

#[test]
fn generator_v1_bytes_are_pinned_independently() {
    let shape = ShapeV1::checked(2, 3, 2).unwrap();
    let inputs = generate_inputs_v1(shape, 0x1234);

    // These constants were derived from the documented SplitMix64-style V1
    // recurrence independently of this Rust implementation. They are a
    // V1-stable regression vector, not values recomputed by the code under test.
    assert_eq!(inputs.a_bits(), [0x0000, 0x4080, 0xbd80, 0x4040]);
    assert_eq!(
        inputs.b_bits(),
        [0xbe80, 0x4040, 0x4040, 0x3e80, 0x4080, 0x3e00]
    );

    // These FP32 bits were calculated independently from the exact dyadic
    // values above and pin the row-major recurrence as well as the generator.
    let validated = validate_evidence_inputs_v1(shape, &inputs.a, &inputs.b).unwrap();
    let output = tiled_gemm_evidence_oracle_v1(validated);
    assert_eq!(
        output
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        [
            0x3f80_0000,
            0x4180_0000,
            0x3f00_0000,
            0x3f44_0000,
            0x413d_0000,
            0x3e40_0000,
        ]
    );
}

#[test]
fn every_generated_value_is_an_exact_allowed_finite_bf16_pattern() {
    let allowed = BF16_INPUT_PATTERN_V1
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for m_tiles in 0..=3 {
        for n_tiles in 0..=3 {
            for k_tiles in 0..=3 {
                let shape = ShapeV1::checked(m_tiles * 16, n_tiles * 16, k_tiles * 16).unwrap();
                for seed in [0, 1, u64::MAX, 0xa5a5_5a5a_c3c3_3c3c] {
                    let inputs = generate_inputs_v1(shape, seed);
                    for value in inputs.a.iter().chain(&inputs.b) {
                        assert!(allowed.contains(&value.to_bits()));
                        assert!(value.is_finite());
                        assert!(!value.is_subnormal());
                        assert_eq!(Bf16::from_f32(value.to_f32()).to_bits(), value.to_bits());
                    }
                }
            }
        }
    }
}

#[test]
fn evidence_validation_rejects_hostile_bf16_classes() {
    let shape = ShapeV1::checked(1, 1, 1).unwrap();
    let cases = [
        (
            0x7fc1,
            EvidenceInputErrorV1::NaNEncoding {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0x7fc1,
            },
        ),
        (
            0xffc1,
            EvidenceInputErrorV1::NaNEncoding {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0xffc1,
            },
        ),
        (
            0x7f80,
            EvidenceInputErrorV1::InfinityEncoding {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0x7f80,
            },
        ),
        (
            0xff80,
            EvidenceInputErrorV1::InfinityEncoding {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0xff80,
            },
        ),
        (
            0x0001,
            EvidenceInputErrorV1::SubnormalEncoding {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0x0001,
            },
        ),
        (
            0x807f,
            EvidenceInputErrorV1::SubnormalEncoding {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0x807f,
            },
        ),
        (
            0x8000,
            EvidenceInputErrorV1::OutsidePinnedAlphabet {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0x8000,
            },
        ),
        (
            0x3f40,
            EvidenceInputErrorV1::OutsidePinnedAlphabet {
                operand: EvidenceOperandV1::A,
                index: 0,
                bits: 0x3f40,
            },
        ),
    ];

    for (bits, expected) in cases {
        let a = [Bf16::from_bits(bits)];
        assert_eq!(
            validate_evidence_inputs_v1(shape, &a, &[Bf16::ONE]),
            Err(expected),
            "bits=0x{bits:04x}"
        );
    }
}

#[test]
fn evidence_validation_admits_exactly_the_pinned_alphabet() {
    let shape = ShapeV1::checked(1, 1, 1).unwrap();
    for bits in 0..=u16::MAX {
        let a = [Bf16::from_bits(bits)];
        let admitted = validate_evidence_inputs_v1(shape, &a, &[Bf16::ONE]).is_ok();
        assert_eq!(
            admitted,
            BF16_INPUT_PATTERN_V1.contains(&bits),
            "bits=0x{bits:04x}"
        );
    }
}

#[test]
fn evidence_validation_reports_operand_and_row_major_index() {
    let shape = ShapeV1::checked(1, 2, 1).unwrap();
    let b = [Bf16::ONE, Bf16::from_bits(0x3f40)];
    assert_eq!(
        validate_evidence_inputs_v1(shape, &[Bf16::ONE], &b),
        Err(EvidenceInputErrorV1::OutsidePinnedAlphabet {
            operand: EvidenceOperandV1::B,
            index: 1,
            bits: 0x3f40,
        })
    );
}

#[test]
fn evidence_validation_rejects_wrong_lengths_before_evaluation() {
    let shape = ShapeV1::checked(2, 3, 2).unwrap();
    let inputs = generate_inputs_v1(shape, 9);
    assert_eq!(
        validate_evidence_inputs_v1(shape, &inputs.a[..3], &inputs.b),
        Err(EvidenceInputErrorV1::WrongLength {
            operand: EvidenceOperandV1::A,
            expected: 4,
            actual: 3,
        })
    );
    assert_eq!(
        validate_evidence_inputs_v1(shape, &inputs.a, &inputs.b[..5]),
        Err(EvidenceInputErrorV1::WrongLength {
            operand: EvidenceOperandV1::B,
            expected: 6,
            actual: 5,
        })
    );

    let empty = ShapeV1::checked(0, u32::MAX, u32::MAX).unwrap();
    assert_eq!(
        validate_evidence_inputs_v1(empty, &[Bf16::NAN], &[]),
        Err(EvidenceInputErrorV1::WrongLength {
            operand: EvidenceOperandV1::A,
            expected: 0,
            actual: 1,
        })
    );
}

#[test]
fn arithmetic_oracle_matches_a_small_known_row_major_product() {
    let shape = ShapeV1::checked(2, 3, 2).unwrap();
    let a = [1.0, 2.0, 3.0, 4.0].map(Bf16::from_f32);
    let b = [5.0, 6.0, 7.0, 8.0, 9.0, 10.0].map(Bf16::from_f32);
    let output = tiled_gemm_arithmetic_oracle_v1(shape, &a, &b).unwrap();
    let expected = [21.0_f32, 24.0, 27.0, 47.0, 54.0, 61.0];
    assert_eq!(
        output
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected.map(f32::to_bits)
    );
}

#[test]
fn zero_k_evidence_produces_only_fp32_positive_zero() {
    for &(m, n) in &[(1, 1), (16, 16), (17, 31), (0, 9), (9, 0)] {
        let shape = ShapeV1::checked(m, n, 0).unwrap();
        let validated = validate_evidence_inputs_v1(shape, &[], &[]).unwrap();
        let output = tiled_gemm_evidence_oracle_v1(validated);
        assert_eq!(output.len(), shape.c_elements());
        assert!(output.iter().all(|value| value.to_bits() == 0));
    }
}

#[test]
fn general_arithmetic_empty_output_ignores_operand_storage() {
    for shape in [
        ShapeV1::checked(0, u32::MAX, u32::MAX).unwrap(),
        ShapeV1::checked(u32::MAX, 0, u32::MAX).unwrap(),
    ] {
        let inputs = generate_inputs_v1(shape, u64::MAX);
        assert!(inputs.a.is_empty());
        assert!(inputs.b.is_empty());

        let sentinel_a = [Bf16::ONE];
        let sentinel_b = [Bf16::NEG_ZERO];
        assert!(
            tiled_gemm_arithmetic_oracle_v1(shape, &sentinel_a, &sentinel_b)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn general_arithmetic_vectors_pin_rounding_order_and_cancellation() {
    let shape = ShapeV1::checked(1, 1, 3).unwrap();
    let one = Bf16::ONE;
    let positive_2_pow_24 = Bf16::from_bits(0x4b80);
    let negative_2_pow_24 = Bf16::from_bits(0xcb80);
    let a = [one; 3];

    // roundTiesToEven loses the unit before cancellation:
    // ((+0 + 2^24) + 1) + -2^24 = +0.
    let rounded_then_cancelled = [positive_2_pow_24, one, negative_2_pow_24];
    let first = tiled_gemm_arithmetic_oracle_v1(shape, &a, &rounded_then_cancelled).unwrap();
    assert_eq!(
        first
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        [0x0000_0000]
    );

    // Changing only recurrence order cancels first and preserves the unit:
    // ((+0 + 2^24) + -2^24) + 1 = 1.
    let cancelled_then_added = [positive_2_pow_24, negative_2_pow_24, one];
    let second = tiled_gemm_arithmetic_oracle_v1(shape, &a, &cancelled_then_added).unwrap();
    assert_eq!(
        second
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        [0x3f80_0000]
    );
}

#[test]
fn general_arithmetic_vectors_pin_signed_zero_accumulation() {
    let shape = ShapeV1::checked(1, 1, 1).unwrap();
    let negative_one = Bf16::from_bits(0xbf80);

    // Both products are negative zero. The recurrence starts at positive zero,
    // so roundTiesToEven addition must produce the pinned positive-zero output.
    for (a, b) in [
        ([Bf16::ZERO], [negative_one]),
        ([Bf16::NEG_ZERO], [Bf16::ONE]),
    ] {
        let output = tiled_gemm_arithmetic_oracle_v1(shape, &a, &b).unwrap();
        assert_eq!(output[0].to_bits(), 0x0000_0000);
    }
}

#[test]
fn arithmetic_oracle_rejects_every_one_element_length_substitution() {
    let shape = ShapeV1::checked(2, 3, 4).unwrap();
    let inputs = generate_inputs_v1(shape, 7);
    assert_eq!(
        tiled_gemm_arithmetic_oracle_v1(shape, &inputs.a[..inputs.a.len() - 1], &inputs.b),
        Err(ArithmeticOracleErrorV1::WrongALength {
            expected: 8,
            actual: 7
        })
    );
    assert_eq!(
        tiled_gemm_arithmetic_oracle_v1(shape, &inputs.a, &inputs.b[..inputs.b.len() - 1]),
        Err(ArithmeticOracleErrorV1::WrongBLength {
            expected: 12,
            actual: 11
        })
    );

    let mut longer_a = inputs.a.clone();
    longer_a.push(Bf16::ZERO);
    assert_eq!(
        tiled_gemm_arithmetic_oracle_v1(shape, &longer_a, &inputs.b),
        Err(ArithmeticOracleErrorV1::WrongALength {
            expected: 8,
            actual: 9
        })
    );
}

#[test]
fn validated_generated_evidence_is_reproducible_over_shape_and_seed_matrix() {
    for &(m, n, k) in &[
        (0, 16, 16),
        (16, 0, 16),
        (1, 1, 0),
        (1, 1, 1),
        (3, 5, 7),
        (16, 16, 16),
        (16, 32, 17),
        (32, 16, 32),
    ] {
        let shape = ShapeV1::checked(m, n, k).unwrap();
        for seed in 0..8 {
            let first_inputs = generate_inputs_v1(shape, seed);
            let second_inputs = generate_inputs_v1(shape, seed);
            let first_validated =
                validate_evidence_inputs_v1(shape, &first_inputs.a, &first_inputs.b).unwrap();
            let second_validated =
                validate_evidence_inputs_v1(shape, &second_inputs.a, &second_inputs.b).unwrap();
            let first = tiled_gemm_evidence_oracle_v1(first_validated);
            let second = tiled_gemm_evidence_oracle_v1(second_validated);
            assert_eq!(
                first
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>(),
                second
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>(),
                "shape=({m},{n},{k}), seed={seed}"
            );
        }
    }
}
