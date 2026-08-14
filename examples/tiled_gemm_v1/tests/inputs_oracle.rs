use std::collections::BTreeSet;

use fe2o3_device::Bf16;
use fe2o3_tiled_gemm_v1::{
    BF16_INPUT_PATTERN_V1, OracleErrorV1, ShapeV1, generate_inputs_v1, tiled_gemm_oracle_v1,
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
fn oracle_matches_a_small_known_row_major_product_bitwise() {
    let shape = ShapeV1::checked(2, 3, 2).unwrap();
    let a = [1.0, 2.0, 3.0, 4.0].map(Bf16::from_f32);
    let b = [5.0, 6.0, 7.0, 8.0, 9.0, 10.0].map(Bf16::from_f32);
    let output = tiled_gemm_oracle_v1(shape, &a, &b).unwrap();
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
fn zero_k_oracle_produces_only_fp32_positive_zero() {
    for &(m, n) in &[(1, 1), (16, 16), (17, 31), (0, 9), (9, 0)] {
        let shape = ShapeV1::checked(m, n, 0).unwrap();
        let output = tiled_gemm_oracle_v1(shape, &[], &[]).unwrap();
        assert_eq!(output.len(), shape.c_elements);
        assert!(output.iter().all(|value| value.to_bits() == 0));
    }
}

#[test]
fn oracle_rejects_every_one_element_length_substitution() {
    let shape = ShapeV1::checked(2, 3, 4).unwrap();
    let inputs = generate_inputs_v1(shape, 7);
    assert_eq!(
        tiled_gemm_oracle_v1(shape, &inputs.a[..inputs.a.len() - 1], &inputs.b),
        Err(OracleErrorV1::WrongALength {
            expected: 8,
            actual: 7
        })
    );
    assert_eq!(
        tiled_gemm_oracle_v1(shape, &inputs.a, &inputs.b[..inputs.b.len() - 1]),
        Err(OracleErrorV1::WrongBLength {
            expected: 12,
            actual: 11
        })
    );

    let mut longer_a = inputs.a.clone();
    longer_a.push(Bf16::ZERO);
    assert_eq!(
        tiled_gemm_oracle_v1(shape, &longer_a, &inputs.b),
        Err(OracleErrorV1::WrongALength {
            expected: 8,
            actual: 9
        })
    );
}

#[test]
fn generated_oracle_is_reproducible_over_shape_and_seed_matrix() {
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
            let first = tiled_gemm_oracle_v1(shape, &first_inputs.a, &first_inputs.b).unwrap();
            let second = tiled_gemm_oracle_v1(shape, &second_inputs.a, &second_inputs.b).unwrap();
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
