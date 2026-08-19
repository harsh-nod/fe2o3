use std::collections::BTreeSet;

use fe2o3_verifier::{
    AuthenticatedGeneralGemmNumericalPolicyV1, GENERAL_GEMM_NUMERICAL_POLICY_SCHEMA_V1,
    GeneralGemmEvidenceIdentityV1, GeneralGemmFloatClassV1,
    GeneralGemmNumericalComparisonPolicyErrorV1, GeneralGemmNumericalComparisonPolicyV1,
    GeneralGemmNumericalEvidenceStatusV1, GeneralGemmNumericalPolicyErrorV1,
    GeneralGemmNumericalPolicyRequestV1, GeneralGemmNumericalStageV1,
    classify_general_gemm_bf16_v1, classify_general_gemm_f32_v1,
    compare_general_gemm_numerical_observation_v1, evaluate_general_gemm_numerical_policy_v1,
    execute_general_gemm_numerical_policy_v1, widen_general_gemm_bf16_v1,
};

const SOURCE: &str = include_str!("../src/general_gemm_numerical_v1.rs");

fn identity(seed: u8) -> GeneralGemmEvidenceIdentityV1 {
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([seed; 32])
}

fn request(offset: u8) -> GeneralGemmNumericalPolicyRequestV1 {
    GeneralGemmNumericalPolicyRequestV1::checked(
        identity(offset + 1),
        identity(offset + 2),
        identity(offset + 3),
        identity(offset + 4),
    )
    .unwrap()
}

fn exact_witness(offset: u8) -> AuthenticatedGeneralGemmNumericalPolicyV1 {
    execute_general_gemm_numerical_policy_v1(
        request(offset),
        GeneralGemmNumericalComparisonPolicyV1::ExactBits,
    )
    .unwrap()
}

#[test]
fn request_rejects_zero_and_cross_domain_identity_reuse() {
    assert!(!request(0).grants_concrete_launch_authority());
    let zero = GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([0; 32]);
    assert_eq!(
        GeneralGemmNumericalPolicyRequestV1::checked(zero, identity(2), identity(3), identity(4)),
        Err(GeneralGemmNumericalPolicyErrorV1::InvalidIdentity)
    );
    assert_eq!(
        GeneralGemmNumericalPolicyRequestV1::checked(
            identity(1),
            identity(2),
            identity(1),
            identity(4)
        ),
        Err(GeneralGemmNumericalPolicyErrorV1::DuplicateIdentity)
    );
}

#[test]
fn bf16_widening_and_classification_cover_every_encoding() {
    let mut classes = [0_usize; 5];
    for bits in 0..=u16::MAX {
        assert_eq!(
            widen_general_gemm_bf16_v1(bits).to_bits(),
            u32::from(bits) << 16
        );
        let class = classify_general_gemm_bf16_v1(bits);
        classes[class as usize - 1] += 1;
        assert_eq!(class, classify_general_gemm_f32_v1(u32::from(bits) << 16));
    }
    assert_eq!(classes, [2, 65_024, 254, 2, 254]);
    assert!(GeneralGemmFloatClassV1::Zero.is_normal_or_zero());
    assert!(GeneralGemmFloatClassV1::Normal.is_normal_or_zero());
    assert!(!GeneralGemmFloatClassV1::Subnormal.is_normal_or_zero());
}

#[test]
fn finite_recurrence_checks_increasing_k_and_alpha_beta_epilogue() {
    let evaluation = evaluate_general_gemm_numerical_policy_v1(
        &[0x3f80, 0x4040],
        &[0x4000, 0x4080],
        8.0,
        0.5,
        0.25,
    )
    .unwrap();
    assert_eq!(evaluation.depth(), 2);
    assert_eq!(evaluation.accumulator_bits(), 14.0_f32.to_bits());
    assert_eq!(evaluation.output_bits(), 9.0_f32.to_bits());
    assert_ne!(evaluation.identity().as_bytes(), &[0; 32]);

    let increasing = evaluate_general_gemm_numerical_policy_v1(
        &[0x4f80, 0xcf80, 0x3f80],
        &[0x3f80; 3],
        0.0,
        1.0,
        0.0,
    )
    .unwrap();
    assert_eq!(increasing.output_bits(), 1.0_f32.to_bits());
    let reassociated =
        widen_general_gemm_bf16_v1(0x4f80) + (widen_general_gemm_bf16_v1(0xcf80) + 1.0);
    assert_eq!(reassociated.to_bits(), 0.0_f32.to_bits());
}

#[test]
fn normal_or_zero_policy_rejects_inputs_and_intermediate_overflow() {
    for (bits, class) in [
        (0x0001, GeneralGemmFloatClassV1::Subnormal),
        (0x7f80, GeneralGemmFloatClassV1::Infinity),
        (0x7fc1, GeneralGemmFloatClassV1::NaN),
    ] {
        assert_eq!(
            evaluate_general_gemm_numerical_policy_v1(&[bits], &[0x3f80], 0.0, 1.0, 0.0),
            Err(GeneralGemmNumericalPolicyErrorV1::UnsupportedValue {
                stage: GeneralGemmNumericalStageV1::AInput,
                index: 0,
                bits: u32::from(bits),
                class,
            })
        );
    }
    assert!(matches!(
        evaluate_general_gemm_numerical_policy_v1(&[0x7f7f], &[0x4000], 0.0, 1.0, 0.0),
        Err(GeneralGemmNumericalPolicyErrorV1::UnsupportedValue {
            stage: GeneralGemmNumericalStageV1::Product,
            class: GeneralGemmFloatClassV1::Infinity,
            ..
        })
    ));
    assert_eq!(
        evaluate_general_gemm_numerical_policy_v1(&[0x3f80], &[], 0.0, 1.0, 0.0),
        Err(GeneralGemmNumericalPolicyErrorV1::LengthMismatch)
    );
}

#[test]
fn comparison_policy_is_checked_and_requires_both_bounded_tests() {
    for (absolute, relative, ulps, error) in [
        (
            -1.0,
            0.0,
            1,
            GeneralGemmNumericalComparisonPolicyErrorV1::InvalidAbsoluteTolerance,
        ),
        (
            0.0,
            f32::from_bits(1),
            1,
            GeneralGemmNumericalComparisonPolicyErrorV1::InvalidRelativeTolerance,
        ),
        (
            0.0,
            0.0,
            1,
            GeneralGemmNumericalComparisonPolicyErrorV1::ZeroNumericalTolerance,
        ),
        (
            1.0,
            0.0,
            0,
            GeneralGemmNumericalComparisonPolicyErrorV1::ZeroUlpTolerance,
        ),
    ] {
        assert_eq!(
            GeneralGemmNumericalComparisonPolicyV1::checked_bounded(absolute, relative, ulps),
            Err(error)
        );
    }

    let bounded = GeneralGemmNumericalComparisonPolicyV1::checked_bounded(1.0, 0.0, 1).unwrap();
    let one_ulp = f32::from_bits(1.0_f32.to_bits() + 1);
    let accepted = compare_general_gemm_numerical_observation_v1(bounded, 1.0, one_ulp).unwrap();
    assert_eq!(accepted.ulp_distance(), 1);
    assert!(!accepted.grants_numerical_refinement());
    assert!(matches!(
        compare_general_gemm_numerical_observation_v1(bounded, 1.0, 1.5),
        Err(GeneralGemmNumericalPolicyErrorV1::ComparisonMismatch { .. })
    ));
    assert!(matches!(
        compare_general_gemm_numerical_observation_v1(
            GeneralGemmNumericalComparisonPolicyV1::ExactBits,
            0.0,
            -0.0
        ),
        Err(GeneralGemmNumericalPolicyErrorV1::ComparisonMismatch { .. })
    ));
}

#[test]
fn witness_binds_compiler_inputs_and_keeps_mfma_boundary_open() {
    let reference = exact_witness(0);
    let optimized = exact_witness(16);
    assert_ne!(reference.identity(), optimized.identity());
    assert_ne!(reference.bf16_closure_identity().as_bytes(), &[0; 32]);
    assert_ne!(reference.mutation_identity().as_bytes(), &[0; 32]);
    assert_eq!(reference.parts().len(), 6);
    let names: BTreeSet<_> = reference.parts().iter().map(|part| part.name()).collect();
    assert_eq!(names.len(), 6);
    assert_eq!(
        reference.parts().last().unwrap().status(),
        GeneralGemmNumericalEvidenceStatusV1::PostLinkMachineConfirmationRequired
    );
    assert!(!reference.exact_real_theorem_is_sufficient());
    assert!(!reference.can_discharge_numerical_contract());
    assert!(
        !reference
            .comparison_policy()
            .can_discharge_exact_numerical_refinement()
    );
    assert_eq!(
        reference.request().symbolic_compilation_identity(),
        identity(1)
    );
}

#[test]
fn source_forbids_trusted_escape_and_names_rounding_mutations() {
    for forbidden in ["unsafe", "assume", "admit", "external_body"] {
        assert!(
            !SOURCE
                .split(|character: char| { !character.is_ascii_alphanumeric() && character != '_' })
                .any(|word| word == forbidden),
            "forbidden escape {forbidden}"
        );
    }
    for required in [
        GENERAL_GEMM_NUMERICAL_POLICY_SCHEMA_V1,
        "Sign-dropping widening",
        "Exact-real equality cannot select",
        "Contracting a multiply/add",
        "PostLinkMachineConfirmationRequired",
        "exact_real_theorem_is_sufficient",
    ] {
        assert!(SOURCE.contains(required), "missing `{required}`");
    }
}
