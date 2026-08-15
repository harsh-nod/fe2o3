use std::collections::BTreeSet;

use fe2o3_tiled_gemm_v1::numerical_contract::{
    ComparisonError, ComparisonPolicy, ComparisonPolicyError, EvaluationStage, GemmInputs,
    GemmSpec, GemmSpecError, HardwareExpectationError, NumericalOperand, SourceEvaluationError,
    UnsupportedValueClass, build_hardware_expectation, compare_hardware, evaluate_source,
    widen_bf16_bits,
};
use fe2o3_tiled_gemm_v1::numerical_vectors::{NumericalVectorKind, deterministic_gemm_vectors};

fn one_by_one_inputs<'a>(a: &'a [u16], b: &'a [u16], c: &'a [f32]) -> GemmInputs<'a> {
    GemmInputs {
        a_bits: a,
        b_bits: b,
        c,
        alpha: 1.0,
        beta: 0.0,
    }
}

#[test]
fn checked_spec_supports_independent_strides_tails_and_zero_k() {
    let padded = GemmSpec::checked(3, 2, 4, 7, 5, 4).unwrap();
    assert_eq!(padded.dimensions(), [3, 2, 4]);
    assert_eq!(padded.strides(), [7, 5, 4]);
    assert_eq!(padded.a_len(), 18);
    assert_eq!(padded.b_len(), 17);
    assert_eq!(padded.c_len(), 10);
    assert_eq!(padded.output_len(), 6);

    let zero_k = GemmSpec::checked(2, 3, 0, 0, 3, 5).unwrap();
    assert_eq!(zero_k.a_len(), 0);
    assert_eq!(zero_k.b_len(), 0);
    assert_eq!(zero_k.c_len(), 8);
    assert_eq!(zero_k.output_len(), 6);
    let c = [2.0, 4.0, 6.0, f32::NAN, f32::NAN, 8.0, 10.0, 12.0];
    let output = evaluate_source(
        zero_k,
        GemmInputs {
            a_bits: &[],
            b_bits: &[],
            c: &c,
            alpha: 7.0,
            beta: 0.5,
        },
    )
    .unwrap();
    assert_eq!(output, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
}

#[test]
fn checked_spec_rejects_short_strides_and_overflow() {
    assert_eq!(
        GemmSpec::checked(2, 3, 4, 3, 3, 3),
        Err(GemmSpecError::StrideTooSmall {
            operand: NumericalOperand::A,
            minimum: 4,
            actual: 3,
        })
    );
    assert_eq!(
        GemmSpec::checked(2, 3, 4, 4, 2, 3),
        Err(GemmSpecError::StrideTooSmall {
            operand: NumericalOperand::B,
            minimum: 3,
            actual: 2,
        })
    );
    assert_eq!(
        GemmSpec::checked(2, 3, 4, 4, 3, 2),
        Err(GemmSpecError::StrideTooSmall {
            operand: NumericalOperand::C,
            minimum: 3,
            actual: 2,
        })
    );
    assert_eq!(
        GemmSpec::checked(usize::MAX, 1, 2, 2, 1, 1),
        Err(GemmSpecError::ExtentOverflow {
            operand: NumericalOperand::A,
        })
    );
    assert_eq!(
        GemmSpec::checked(usize::MAX, 2, 0, 0, 2, 2),
        Err(GemmSpecError::ExtentOverflow {
            operand: NumericalOperand::C,
        })
    );
}

#[test]
fn source_oracle_requires_exact_accessed_extents() {
    let spec = GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        evaluate_source(spec, one_by_one_inputs(&[], &[0x3f80], &[0.0])),
        Err(SourceEvaluationError::WrongLength {
            operand: NumericalOperand::A,
            expected: 1,
            actual: 0,
        })
    );
    assert_eq!(
        evaluate_source(spec, one_by_one_inputs(&[0x3f80], &[0x3f80, 0], &[0.0])),
        Err(SourceEvaluationError::WrongLength {
            operand: NumericalOperand::B,
            expected: 1,
            actual: 2,
        })
    );
}

#[test]
fn bf16_widening_is_exact_for_every_encoding() {
    for bits in 0..=u16::MAX {
        assert_eq!(widen_bf16_bits(bits).to_bits(), u32::from(bits) << 16);
    }
    assert_eq!(widen_bf16_bits(0x0000).to_bits(), 0x0000_0000);
    assert_eq!(widen_bf16_bits(0x8000).to_bits(), 0x8000_0000);
    assert_eq!(widen_bf16_bits(0x3f80), 1.0);
    assert!(widen_bf16_bits(0x7fc1).is_nan());
}

#[test]
fn source_oracle_uses_increasing_k_separate_fp32_ops_and_alpha_beta() {
    let spec = GemmSpec::checked(2, 2, 3, 3, 2, 3).unwrap();
    let a = [0x3f80, 0x4000, 0x4040, 0x4080, 0x40a0, 0x40c0];
    let b = [0x40a0, 0x40c0, 0x40e0, 0x4100, 0x4110, 0x4120];
    let c = [4.0, -8.0, f32::NAN, 12.0, -16.0];
    let output = evaluate_source(
        spec,
        GemmInputs {
            a_bits: &a,
            b_bits: &b,
            c: &c,
            alpha: 0.5,
            beta: 0.25,
        },
    )
    .unwrap();
    assert_eq!(
        output
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        [0x41c0_0000, 0x41c0_0000, 0x4266_0000, 0x4268_0000,]
    );
}

#[test]
fn source_semantics_remain_defined_for_exceptional_ieee_inputs() {
    let spec = GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap();
    let nan = evaluate_source(spec, one_by_one_inputs(&[0x7fc1], &[0x3f80], &[0.0])).unwrap();
    assert!(nan[0].is_nan());

    let infinity = evaluate_source(spec, one_by_one_inputs(&[0x7f80], &[0x3f80], &[0.0])).unwrap();
    assert_eq!(infinity, [f32::INFINITY]);

    let subnormal = evaluate_source(spec, one_by_one_inputs(&[0x0080], &[0x3f00], &[0.0])).unwrap();
    assert!(subnormal[0].is_subnormal());
}

#[test]
fn finite_policy_rejects_every_exceptional_bf16_class_with_location() {
    let spec = GemmSpec::checked(1, 2, 1, 1, 2, 2).unwrap();
    let cases = [
        (0x0001, UnsupportedValueClass::Subnormal),
        (0x8001, UnsupportedValueClass::Subnormal),
        (0x7f80, UnsupportedValueClass::Infinity),
        (0xff80, UnsupportedValueClass::Infinity),
        (0x7fc1, UnsupportedValueClass::NaN),
        (0xff81, UnsupportedValueClass::NaN),
    ];
    for (bits, class) in cases {
        let error = build_hardware_expectation(
            spec,
            GemmInputs {
                a_bits: &[0x3f80],
                b_bits: &[0x3f80, bits],
                c: &[0.0, 0.0],
                alpha: 1.0,
                beta: 0.0,
            },
            ComparisonPolicy::ExactBits,
        )
        .unwrap_err();
        assert_eq!(
            error,
            HardwareExpectationError::UnsupportedBf16 {
                operand: NumericalOperand::B,
                index: 1,
                bits,
                class,
            }
        );
        assert!(error.to_string().contains("B[1]"));
    }
}

#[test]
fn finite_policy_rejects_exceptional_c_and_coefficients() {
    let spec = GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap();
    let cases = [
        (
            NumericalOperand::C,
            f32::from_bits(1),
            UnsupportedValueClass::Subnormal,
        ),
        (
            NumericalOperand::C,
            f32::INFINITY,
            UnsupportedValueClass::Infinity,
        ),
        (NumericalOperand::C, f32::NAN, UnsupportedValueClass::NaN),
        (
            NumericalOperand::Alpha,
            f32::NEG_INFINITY,
            UnsupportedValueClass::Infinity,
        ),
        (
            NumericalOperand::Beta,
            f32::from_bits(0x8000_0001),
            UnsupportedValueClass::Subnormal,
        ),
    ];
    for (operand, value, class) in cases {
        let c = if operand == NumericalOperand::C {
            value
        } else {
            0.0
        };
        let alpha = if operand == NumericalOperand::Alpha {
            value
        } else {
            1.0
        };
        let beta = if operand == NumericalOperand::Beta {
            value
        } else {
            0.0
        };
        assert_eq!(
            build_hardware_expectation(
                spec,
                GemmInputs {
                    a_bits: &[0x3f80],
                    b_bits: &[0x3f80],
                    c: &[c],
                    alpha,
                    beta,
                },
                ComparisonPolicy::ExactBits,
            ),
            Err(HardwareExpectationError::UnsupportedF32 {
                operand,
                index: 0,
                bits: value.to_bits(),
                class,
            })
        );
    }
}

#[test]
fn finite_policy_rejects_overflow_and_subnormal_intermediates() {
    let spec = GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap();
    let overflow = build_hardware_expectation(
        spec,
        one_by_one_inputs(&[0x7f7f], &[0x4000], &[0.0]),
        ComparisonPolicy::ExactBits,
    )
    .unwrap_err();
    assert!(matches!(
        overflow,
        HardwareExpectationError::UnsupportedIntermediate {
            row: 0,
            column: 0,
            depth: Some(0),
            stage: EvaluationStage::Product,
            class: UnsupportedValueClass::Infinity,
            ..
        }
    ));

    let underflow = build_hardware_expectation(
        spec,
        one_by_one_inputs(&[0x0080], &[0x3f00], &[0.0]),
        ComparisonPolicy::ExactBits,
    )
    .unwrap_err();
    assert!(matches!(
        underflow,
        HardwareExpectationError::UnsupportedIntermediate {
            stage: EvaluationStage::Product,
            class: UnsupportedValueClass::Subnormal,
            ..
        }
    ));
}

#[test]
fn comparison_policy_validation_fails_closed() {
    assert_eq!(
        ComparisonPolicy::bounded(-0.0, 0.0, 1),
        Err(ComparisonPolicyError::InvalidAbsoluteTolerance(
            (-0.0_f32).to_bits()
        ))
    );
    assert_eq!(
        ComparisonPolicy::bounded(f32::NAN, 0.0, 1),
        Err(ComparisonPolicyError::InvalidAbsoluteTolerance(
            f32::NAN.to_bits()
        ))
    );
    assert_eq!(
        ComparisonPolicy::bounded(0.0, f32::INFINITY, 1),
        Err(ComparisonPolicyError::InvalidRelativeTolerance(
            f32::INFINITY.to_bits()
        ))
    );
    assert_eq!(
        ComparisonPolicy::bounded(0.0, 0.0, 1),
        Err(ComparisonPolicyError::ZeroNumericTolerance)
    );
    assert_eq!(
        ComparisonPolicy::bounded(1.0e-6, 0.0, 0),
        Err(ComparisonPolicyError::ZeroUlpTolerance)
    );
}

#[test]
fn exact_policy_preserves_signed_zero_and_rejects_substituted_output() {
    let spec = GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap();
    let expectation = build_hardware_expectation(
        spec,
        one_by_one_inputs(&[0x0000], &[0x3f80], &[0.0]),
        ComparisonPolicy::ExactBits,
    )
    .unwrap();
    assert_eq!(expectation.expected()[0].to_bits(), 0);
    assert!(matches!(
        compare_hardware(&expectation, &[-0.0]),
        Err(ComparisonError::Mismatch {
            expected_bits: 0,
            actual_bits: 0x8000_0000,
            ..
        })
    ));
    assert!(matches!(
        compare_hardware(&expectation, &[f32::from_bits(1)]),
        Err(ComparisonError::Mismatch { .. })
    ));
}

#[test]
fn bounded_policy_requires_numeric_and_ulp_bounds() {
    let spec = GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap();
    let policy = ComparisonPolicy::bounded(f32::MIN_POSITIVE, 0.0, 1).unwrap();
    let expectation = build_hardware_expectation(
        spec,
        one_by_one_inputs(&[0x0000], &[0x3f80], &[0.0]),
        policy,
    )
    .unwrap();
    let report = compare_hardware(&expectation, &[-0.0]).unwrap();
    assert_eq!(report.max_abs_error, 0.0);
    assert_eq!(report.max_ulp_error, 1);

    let expectation = build_hardware_expectation(
        spec,
        one_by_one_inputs(&[0x3f80], &[0x4000], &[0.0]),
        ComparisonPolicy::bounded(1.0, 1.0, 1).unwrap(),
    )
    .unwrap();
    let too_many_ulps = f32::from_bits(2.0_f32.to_bits() + 2);
    assert!(matches!(
        compare_hardware(&expectation, &[too_many_ulps]),
        Err(ComparisonError::Mismatch { ulp_error: 2, .. })
    ));
}

#[test]
fn comparison_rejects_length_and_nonfinite_observation() {
    let spec = GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap();
    let expectation = build_hardware_expectation(
        spec,
        one_by_one_inputs(&[0x3f80], &[0x3f80], &[0.0]),
        ComparisonPolicy::ExactBits,
    )
    .unwrap();
    assert_eq!(
        compare_hardware(&expectation, &[]),
        Err(ComparisonError::WrongObservedLength {
            expected: 1,
            actual: 0,
        })
    );
    assert_eq!(
        compare_hardware(&expectation, &[f32::NAN]),
        Err(ComparisonError::NonFiniteObservation {
            index: 0,
            bits: f32::NAN.to_bits(),
        })
    );
}

#[test]
fn deterministic_corpus_covers_every_required_case_once() {
    let vectors = deterministic_gemm_vectors();
    let kinds = vectors
        .iter()
        .map(|vector| vector.kind())
        .collect::<BTreeSet<_>>();
    assert_eq!(vectors.len(), 9);
    assert_eq!(kinds.len(), 9);
    for kind in [
        NumericalVectorKind::Zero,
        NumericalVectorKind::Identity,
        NumericalVectorKind::Dyadic,
        NumericalVectorKind::Cancellation,
        NumericalVectorKind::Randomized,
        NumericalVectorKind::PaddedStride,
        NumericalVectorKind::Tail,
        NumericalVectorKind::NonzeroC,
        NumericalVectorKind::AdversarialFinite,
    ] {
        assert!(kinds.contains(&kind));
    }

    let names = vectors
        .iter()
        .map(|vector| vector.name())
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), vectors.len());
}

#[test]
fn deterministic_corpus_repeats_bit_for_bit_and_all_cases_self_compare() {
    let first = deterministic_gemm_vectors();
    let second = deterministic_gemm_vectors();
    assert_eq!(first.len(), second.len());
    for (left, right) in first.iter().zip(&second) {
        assert_eq!(left.name(), right.name());
        assert_eq!(left.kind(), right.kind());
        assert_eq!(left.spec(), right.spec());
        assert_eq!(left.a_bits(), right.a_bits());
        assert_eq!(left.b_bits(), right.b_bits());
        assert_eq!(
            left.c()
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            right
                .c()
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(left.alpha().to_bits(), right.alpha().to_bits());
        assert_eq!(left.beta().to_bits(), right.beta().to_bits());
        assert_eq!(left.policy(), right.policy());
    }

    for vector in first {
        let expectation = vector
            .expectation()
            .unwrap_or_else(|error| panic!("{}: {error}", vector.name()));
        let report = compare_hardware(&expectation, expectation.expected()).unwrap();
        assert_eq!(report.outputs, vector.spec().output_len());
        assert_eq!(report.max_abs_error, 0.0);
        assert_eq!(report.max_rel_error, 0.0);
        assert_eq!(report.max_ulp_error, 0);
    }
}

#[test]
fn padded_values_are_not_logical_inputs() {
    let vector = deterministic_gemm_vectors()
        .into_iter()
        .find(|vector| vector.kind() == NumericalVectorKind::PaddedStride)
        .unwrap();
    assert!(vector.a_bits().contains(&0x7fc1));
    assert!(vector.b_bits().contains(&0x7f80));
    assert!(vector.c().iter().any(|value| value.is_nan()));
    vector.expectation().unwrap();
}

#[test]
fn corpus_expected_bits_have_a_pinned_debug_release_digest() {
    fn append(mut hash: u64, bytes: &[u8]) -> u64 {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    let mut hash = 0xcbf2_9ce4_8422_2325;
    for vector in deterministic_gemm_vectors() {
        hash = append(hash, vector.name().as_bytes());
        for value in vector.spec().dimensions() {
            hash = append(hash, &value.to_le_bytes());
        }
        for value in vector.spec().strides() {
            hash = append(hash, &value.to_le_bytes());
        }
        for bits in vector.a_bits().iter().chain(vector.b_bits()) {
            hash = append(hash, &bits.to_le_bytes());
        }
        for value in vector.c() {
            hash = append(hash, &value.to_bits().to_le_bytes());
        }
        hash = append(hash, &vector.alpha().to_bits().to_le_bytes());
        hash = append(hash, &vector.beta().to_bits().to_le_bytes());
        let expectation = vector.expectation().unwrap();
        for value in expectation.expected() {
            hash = append(hash, &value.to_bits().to_le_bytes());
        }
    }
    assert_eq!(hash, 0x6131_fb79_5a3c_1c51);
}
