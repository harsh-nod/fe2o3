//! Executable BF16/FP32 numerical-policy witness for issue #138 GEMM.
//!
//! This module closes no machine property by itself. It binds an exact
//! executable policy to compiler identities, exhaustively checks BF16 widening
//! and classification, and runs mutation-sensitive FP32 recurrence checks.
//! Actual MFMA instruction semantics and source-to-machine correspondence remain
//! a required post-link input to the final verifier join.

use core::fmt;
use std::hint::black_box;

use sha2::{Digest as _, Sha256};

use crate::GeneralGemmEvidenceIdentityV1;

/// Maximum dot-product depth admitted by one policy evaluation.
pub const MAX_GENERAL_GEMM_NUMERICAL_DEPTH_V1: usize = 1 << 20;
/// Stable schema for the shared finite BF16/FP32 policy.
pub const GENERAL_GEMM_NUMERICAL_POLICY_SCHEMA_V1: &str =
    "fe2o3.general-gemm.bf16-f32-numerical-policy.v1";

const WITNESS_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.symbolic-numerical-witness.v1\0";
const BF16_CLOSURE_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.bf16-closure.v1\0";
const MUTATION_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.numerical-mutations.v1\0";
const EVALUATION_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.numerical-evaluation.v1\0";

/// IEEE class used by the finite normal-or-zero policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmFloatClassV1 {
    /// Positive or negative zero.
    Zero = 1,
    /// A finite nonzero normal number.
    Normal = 2,
    /// A finite nonzero subnormal number, rejected by this policy.
    Subnormal = 3,
    /// Positive or negative infinity, rejected by this policy.
    Infinity = 4,
    /// A quiet or signaling NaN, rejected by this policy.
    NaN = 5,
}

impl GeneralGemmFloatClassV1 {
    /// Returns whether the finite hardware policy admits this class.
    pub const fn is_normal_or_zero(self) -> bool {
        matches!(self, Self::Zero | Self::Normal)
    }
}

/// Classifies one BF16 encoding without first converting it to FP32.
pub const fn classify_general_gemm_bf16_v1(bits: u16) -> GeneralGemmFloatClassV1 {
    let exponent = bits & 0x7f80;
    let fraction = bits & 0x007f;
    if exponent == 0 {
        if fraction == 0 {
            GeneralGemmFloatClassV1::Zero
        } else {
            GeneralGemmFloatClassV1::Subnormal
        }
    } else if exponent == 0x7f80 {
        if fraction == 0 {
            GeneralGemmFloatClassV1::Infinity
        } else {
            GeneralGemmFloatClassV1::NaN
        }
    } else {
        GeneralGemmFloatClassV1::Normal
    }
}

/// Classifies one FP32 encoding without host floating-point comparisons.
pub const fn classify_general_gemm_f32_v1(bits: u32) -> GeneralGemmFloatClassV1 {
    let exponent = bits & 0x7f80_0000;
    let fraction = bits & 0x007f_ffff;
    if exponent == 0 {
        if fraction == 0 {
            GeneralGemmFloatClassV1::Zero
        } else {
            GeneralGemmFloatClassV1::Subnormal
        }
    } else if exponent == 0x7f80_0000 {
        if fraction == 0 {
            GeneralGemmFloatClassV1::Infinity
        } else {
            GeneralGemmFloatClassV1::NaN
        }
    } else {
        GeneralGemmFloatClassV1::Normal
    }
}

/// Widens BF16 to FP32 exactly by filling the low sixteen fraction bits with zero.
pub const fn widen_general_gemm_bf16_v1(bits: u16) -> f32 {
    f32::from_bits((bits as u32) << 16)
}

/// Exact comparison policy retained in one numerical witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmNumericalComparisonPolicyV1 {
    /// Require identical FP32 encodings, including signed zero.
    ExactBits,
    /// Require an absolute/relative envelope and an independent ULP bound.
    Bounded {
        /// Nonnegative, normal-or-zero absolute tolerance bits.
        max_abs_bits: u32,
        /// Nonnegative, normal-or-zero relative tolerance bits.
        max_rel_bits: u32,
        /// Positive maximum ordered FP32 encoding distance.
        max_ulps: u32,
    },
}

/// A requested bounded comparison policy is malformed or vacuous.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmNumericalComparisonPolicyErrorV1 {
    /// Absolute tolerance is negative or not normal-or-zero.
    InvalidAbsoluteTolerance,
    /// Relative tolerance is negative or not normal-or-zero.
    InvalidRelativeTolerance,
    /// Both numerical tolerances are zero.
    ZeroNumericalTolerance,
    /// The independent ULP limit is zero.
    ZeroUlpTolerance,
}

impl GeneralGemmNumericalComparisonPolicyV1 {
    /// Checks a bounded observation policy. Bounded comparison remains
    /// observation evidence and cannot prove exact source recurrence.
    pub fn checked_bounded(
        max_abs: f32,
        max_rel: f32,
        max_ulps: u32,
    ) -> Result<Self, GeneralGemmNumericalComparisonPolicyErrorV1> {
        let valid = |value: f32| {
            !value.is_sign_negative()
                && classify_general_gemm_f32_v1(value.to_bits()).is_normal_or_zero()
        };
        if !valid(max_abs) {
            return Err(GeneralGemmNumericalComparisonPolicyErrorV1::InvalidAbsoluteTolerance);
        }
        if !valid(max_rel) {
            return Err(GeneralGemmNumericalComparisonPolicyErrorV1::InvalidRelativeTolerance);
        }
        if max_abs == 0.0 && max_rel == 0.0 {
            return Err(GeneralGemmNumericalComparisonPolicyErrorV1::ZeroNumericalTolerance);
        }
        if max_ulps == 0 {
            return Err(GeneralGemmNumericalComparisonPolicyErrorV1::ZeroUlpTolerance);
        }
        Ok(Self::Bounded {
            max_abs_bits: max_abs.to_bits(),
            max_rel_bits: max_rel.to_bits(),
            max_ulps,
        })
    }

    /// Bounded observations never establish exact numerical refinement.
    pub const fn can_discharge_exact_numerical_refinement(self) -> bool {
        false
    }
}

/// Symbolic compiler identities bound by the parameterized numerical witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalPolicyRequestV1 {
    symbolic_compilation_identity: GeneralGemmEvidenceIdentityV1,
    symbolic_plan_identity: GeneralGemmEvidenceIdentityV1,
    symbolic_kir_identity: GeneralGemmEvidenceIdentityV1,
    numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmNumericalPolicyRequestV1 {
    /// Requires four nonzero, domain-distinct compiler identities.
    pub fn checked(
        symbolic_compilation_identity: GeneralGemmEvidenceIdentityV1,
        symbolic_plan_identity: GeneralGemmEvidenceIdentityV1,
        symbolic_kir_identity: GeneralGemmEvidenceIdentityV1,
        numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
    ) -> Result<Self, GeneralGemmNumericalPolicyErrorV1> {
        let identities = [
            symbolic_compilation_identity,
            symbolic_plan_identity,
            symbolic_kir_identity,
            numerical_policy_identity,
        ];
        if identities
            .iter()
            .any(|identity| identity.as_bytes() == &[0; 32])
        {
            return Err(GeneralGemmNumericalPolicyErrorV1::InvalidIdentity);
        }
        if identities
            .iter()
            .enumerate()
            .any(|(index, identity)| identities[..index].contains(identity))
        {
            return Err(GeneralGemmNumericalPolicyErrorV1::DuplicateIdentity);
        }
        Ok(Self {
            symbolic_compilation_identity,
            symbolic_plan_identity,
            symbolic_kir_identity,
            numerical_policy_identity,
        })
    }

    /// Returns the aggregate symbolic compilation identity.
    pub const fn symbolic_compilation_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.symbolic_compilation_identity
    }

    /// Returns the canonical symbolic plan-schema identity.
    pub const fn symbolic_plan_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.symbolic_plan_identity
    }

    /// Returns the canonical symbolic KIR-template identity.
    pub const fn symbolic_kir_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.symbolic_kir_identity
    }

    /// Returns the frontend-authenticated numerical-policy identity.
    pub const fn numerical_policy_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.numerical_policy_identity
    }

    /// Symbolic numerical policy evidence never authorizes concrete operands.
    pub const fn grants_concrete_launch_authority(self) -> bool {
        false
    }
}

/// Operation stage rejected by finite normal-or-zero policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmNumericalStageV1 {
    /// BF16 A input.
    AInput,
    /// BF16 B input.
    BInput,
    /// Initial FP32 C value.
    CInput,
    /// FP32 alpha coefficient.
    AlphaInput,
    /// FP32 beta coefficient.
    BetaInput,
    /// Exactly widened BF16 product.
    Product,
    /// Increasing-K FP32 accumulator update.
    Accumulation,
    /// Separate alpha multiplication.
    AlphaScale,
    /// Separate beta multiplication.
    BetaScale,
    /// Separate epilogue addition.
    EpilogueAdd,
    /// Observed FP32 result.
    Observation,
}

/// Shared numerical-policy construction or evaluation failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmNumericalPolicyErrorV1 {
    /// A required compiler identity was zero.
    InvalidIdentity,
    /// Two independently owned identity domains reused the same bytes.
    DuplicateIdentity,
    /// A/B dot inputs have different lengths.
    LengthMismatch,
    /// The configured dot depth exceeds the hard bound.
    DepthLimit,
    /// A value was not finite normal-or-zero at a required stage.
    UnsupportedValue {
        /// Rejected stage.
        stage: GeneralGemmNumericalStageV1,
        /// Element/depth index, or zero for a scalar.
        index: usize,
        /// Exact rejected encoding in the low bits.
        bits: u32,
        /// Rejected IEEE class.
        class: GeneralGemmFloatClassV1,
    },
    /// The internal mutation-sensitive policy checks changed unexpectedly.
    PolicyMutationMismatch,
    /// An observed result did not satisfy the sealed comparison policy.
    ComparisonMismatch {
        /// Expected FP32 encoding.
        expected_bits: u32,
        /// Observed FP32 encoding.
        actual_bits: u32,
        /// Ordered finite FP32 encoding distance.
        ulp_distance: u32,
    },
}

impl fmt::Display for GeneralGemmNumericalPolicyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "general GEMM numerical policy failed: {self:?}")
    }
}

impl std::error::Error for GeneralGemmNumericalPolicyErrorV1 {}

/// One finite increasing-K dot and alpha/beta epilogue evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalEvaluationV1 {
    accumulator_bits: u32,
    output_bits: u32,
    depth: usize,
    identity: GeneralGemmEvidenceIdentityV1,
}

/// Successful non-authoritative comparison of one finite observed result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalComparisonV1 {
    expected_bits: u32,
    actual_bits: u32,
    ulp_distance: u32,
}

impl GeneralGemmNumericalComparisonV1 {
    /// Returns the expected FP32 encoding.
    pub const fn expected_bits(self) -> u32 {
        self.expected_bits
    }

    /// Returns the observed FP32 encoding.
    pub const fn actual_bits(self) -> u32 {
        self.actual_bits
    }

    /// Returns the ordered finite FP32 encoding distance.
    pub const fn ulp_distance(self) -> u32 {
        self.ulp_distance
    }

    /// A finite-corpus comparison is never universal numerical proof.
    pub const fn grants_numerical_refinement(self) -> bool {
        false
    }
}

/// Compares one caller-reported finite result with the sealed policy. This
/// authenticates no observer and grants no proof or hardware authority.
pub fn compare_general_gemm_numerical_observation_v1(
    policy: GeneralGemmNumericalComparisonPolicyV1,
    expected: f32,
    actual: f32,
) -> Result<GeneralGemmNumericalComparisonV1, GeneralGemmNumericalPolicyErrorV1> {
    require_f32_class(expected, GeneralGemmNumericalStageV1::Observation, 0)?;
    require_f32_class(actual, GeneralGemmNumericalStageV1::Observation, 0)?;
    let ulp_distance = ordered_f32(expected.to_bits()).abs_diff(ordered_f32(actual.to_bits()));
    let accepted = match policy {
        GeneralGemmNumericalComparisonPolicyV1::ExactBits => expected.to_bits() == actual.to_bits(),
        GeneralGemmNumericalComparisonPolicyV1::Bounded {
            max_abs_bits,
            max_rel_bits,
            max_ulps,
        } => {
            let absolute_error = (actual - expected).abs();
            let threshold =
                f32::from_bits(max_abs_bits) + f32::from_bits(max_rel_bits) * expected.abs();
            expected.to_bits() == actual.to_bits()
                || (threshold.is_finite()
                    && absolute_error <= threshold
                    && ulp_distance <= max_ulps)
        }
    };
    if !accepted {
        return Err(GeneralGemmNumericalPolicyErrorV1::ComparisonMismatch {
            expected_bits: expected.to_bits(),
            actual_bits: actual.to_bits(),
            ulp_distance,
        });
    }
    Ok(GeneralGemmNumericalComparisonV1 {
        expected_bits: expected.to_bits(),
        actual_bits: actual.to_bits(),
        ulp_distance,
    })
}

impl GeneralGemmNumericalEvaluationV1 {
    /// Returns the exact final FP32 accumulator encoding.
    pub const fn accumulator_bits(self) -> u32 {
        self.accumulator_bits
    }

    /// Returns the exact alpha/beta epilogue output encoding.
    pub const fn output_bits(self) -> u32 {
        self.output_bits
    }

    /// Returns the increasing-K depth consumed.
    pub const fn depth(self) -> usize {
        self.depth
    }

    /// Returns the domain-separated complete evaluation identity.
    pub const fn identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }
}

/// Evaluates the shared source policy with separate FP32 multiply and add
/// operations. It does not claim an emitted MFMA follows this recurrence.
pub fn evaluate_general_gemm_numerical_policy_v1(
    a: &[u16],
    b: &[u16],
    c: f32,
    alpha: f32,
    beta: f32,
) -> Result<GeneralGemmNumericalEvaluationV1, GeneralGemmNumericalPolicyErrorV1> {
    if a.len() != b.len() {
        return Err(GeneralGemmNumericalPolicyErrorV1::LengthMismatch);
    }
    if a.len() > MAX_GENERAL_GEMM_NUMERICAL_DEPTH_V1 {
        return Err(GeneralGemmNumericalPolicyErrorV1::DepthLimit);
    }
    require_f32_class(c, GeneralGemmNumericalStageV1::CInput, 0)?;
    require_f32_class(alpha, GeneralGemmNumericalStageV1::AlphaInput, 0)?;
    require_f32_class(beta, GeneralGemmNumericalStageV1::BetaInput, 0)?;

    let mut accumulator = 0.0_f32;
    for (index, (a_bits, b_bits)) in a.iter().copied().zip(b.iter().copied()).enumerate() {
        require_bf16_class(a_bits, GeneralGemmNumericalStageV1::AInput, index)?;
        require_bf16_class(b_bits, GeneralGemmNumericalStageV1::BInput, index)?;
        let product = separate_f32_mul(
            widen_general_gemm_bf16_v1(a_bits),
            widen_general_gemm_bf16_v1(b_bits),
        );
        require_f32_class(product, GeneralGemmNumericalStageV1::Product, index)?;
        accumulator = separate_f32_add(accumulator, product);
        require_f32_class(
            accumulator,
            GeneralGemmNumericalStageV1::Accumulation,
            index,
        )?;
    }
    let alpha_scaled = separate_f32_mul(alpha, accumulator);
    require_f32_class(alpha_scaled, GeneralGemmNumericalStageV1::AlphaScale, 0)?;
    let beta_scaled = separate_f32_mul(beta, c);
    require_f32_class(beta_scaled, GeneralGemmNumericalStageV1::BetaScale, 0)?;
    let output = separate_f32_add(alpha_scaled, beta_scaled);
    require_f32_class(output, GeneralGemmNumericalStageV1::EpilogueAdd, 0)?;

    let mut hasher = Sha256::new();
    hasher.update(EVALUATION_DOMAIN_V1);
    hasher.update((a.len() as u64).to_le_bytes());
    for bits in a {
        hasher.update(bits.to_le_bytes());
    }
    for bits in b {
        hasher.update(bits.to_le_bytes());
    }
    for bits in [c.to_bits(), alpha.to_bits(), beta.to_bits()] {
        hasher.update(bits.to_le_bytes());
    }
    hasher.update(accumulator.to_bits().to_le_bytes());
    hasher.update(output.to_bits().to_le_bytes());
    Ok(GeneralGemmNumericalEvaluationV1 {
        accumulator_bits: accumulator.to_bits(),
        output_bits: output.to_bits(),
        depth: a.len(),
        identity: GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into()),
    })
}

/// Evidence level of one shared numerical-policy part.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmNumericalEvidenceStatusV1 {
    /// Every BF16 bit pattern was checked against exact widening/classification.
    ExhaustiveBf16Checked,
    /// Executable separate-rounding policy and mutations were checked.
    ExecutablePolicyChecked,
    /// Requires exact post-link opcode, mode, and reduction-order inspection.
    PostLinkMachineConfirmationRequired,
}

/// One independently named numerical-policy result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalEvidencePartV1 {
    name: &'static str,
    status: GeneralGemmNumericalEvidenceStatusV1,
}

impl GeneralGemmNumericalEvidencePartV1 {
    /// Returns the stable evidence-part name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the exact authority boundary.
    pub const fn status(self) -> GeneralGemmNumericalEvidenceStatusV1 {
        self.status
    }
}

/// Privately constructed, compilation-bound shared numerical-policy witness.
///
/// This value is not `Clone`. It grants no artifact, publication, load, launch,
/// exact numerical-refinement, or machine-refinement authority.
#[derive(Debug)]
#[must_use = "numerical policy must be joined to post-link machine refinement"]
pub struct AuthenticatedGeneralGemmNumericalPolicyV1 {
    request: GeneralGemmNumericalPolicyRequestV1,
    comparison: GeneralGemmNumericalComparisonPolicyV1,
    identity: GeneralGemmEvidenceIdentityV1,
    bf16_closure_identity: GeneralGemmEvidenceIdentityV1,
    mutation_identity: GeneralGemmEvidenceIdentityV1,
    parts: [GeneralGemmNumericalEvidencePartV1; 6],
}

impl AuthenticatedGeneralGemmNumericalPolicyV1 {
    /// Returns exact compiler identity bindings.
    pub const fn request(&self) -> GeneralGemmNumericalPolicyRequestV1 {
        self.request
    }

    /// Returns the sealed comparison policy.
    pub const fn comparison_policy(&self) -> GeneralGemmNumericalComparisonPolicyV1 {
        self.comparison
    }

    /// Returns the complete numerical-policy witness identity.
    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    /// Returns the exhaustive BF16 check identity.
    pub const fn bf16_closure_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.bf16_closure_identity
    }

    /// Returns the mutation-sensitive FP32 policy-check identity.
    pub const fn mutation_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.mutation_identity
    }

    /// Returns all independently named policy parts.
    pub const fn parts(&self) -> &[GeneralGemmNumericalEvidencePartV1; 6] {
        &self.parts
    }

    /// An exact-real recurrence theorem does not establish FP32/MFMA rounding.
    pub const fn exact_real_theorem_is_sufficient(&self) -> bool {
        false
    }

    /// Post-link machine inspection is always required before final admission.
    pub const fn can_discharge_numerical_contract(&self) -> bool {
        false
    }
}

/// Runs exhaustive BF16 and mutation-sensitive FP32 policy checks and binds
/// their outputs to one runtime-parameterized symbolic compilation request.
pub fn execute_general_gemm_numerical_policy_v1(
    request: GeneralGemmNumericalPolicyRequestV1,
    comparison: GeneralGemmNumericalComparisonPolicyV1,
) -> Result<AuthenticatedGeneralGemmNumericalPolicyV1, GeneralGemmNumericalPolicyErrorV1> {
    let bf16_closure_identity = check_bf16_closure()?;
    let mutation_identity = check_policy_mutations()?;
    let mut hasher = Sha256::new();
    hasher.update(WITNESS_DOMAIN_V1);
    hasher.update(GENERAL_GEMM_NUMERICAL_POLICY_SCHEMA_V1.as_bytes());
    for identity in [
        request.symbolic_compilation_identity,
        request.symbolic_plan_identity,
        request.symbolic_kir_identity,
        request.numerical_policy_identity,
        bf16_closure_identity,
        mutation_identity,
    ] {
        hasher.update(identity.as_bytes());
    }
    encode_comparison(&mut hasher, comparison);
    let identity = GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into());
    Ok(AuthenticatedGeneralGemmNumericalPolicyV1 {
        request,
        comparison,
        identity,
        bf16_closure_identity,
        mutation_identity,
        parts: [
            part(
                "bf16_exact_widening_and_classification",
                GeneralGemmNumericalEvidenceStatusV1::ExhaustiveBf16Checked,
            ),
            part(
                "finite_normal_or_zero_inputs_and_intermediates",
                GeneralGemmNumericalEvidenceStatusV1::ExecutablePolicyChecked,
            ),
            part(
                "increasing_k_separate_fp32_rounding",
                GeneralGemmNumericalEvidenceStatusV1::ExecutablePolicyChecked,
            ),
            part(
                "alpha_beta_separate_fp32_epilogue",
                GeneralGemmNumericalEvidenceStatusV1::ExecutablePolicyChecked,
            ),
            part(
                "sealed_output_comparison_policy",
                GeneralGemmNumericalEvidenceStatusV1::ExecutablePolicyChecked,
            ),
            part(
                "gfx942_mfma_machine_refinement",
                GeneralGemmNumericalEvidenceStatusV1::PostLinkMachineConfirmationRequired,
            ),
        ],
    })
}

fn check_bf16_closure() -> Result<GeneralGemmEvidenceIdentityV1, GeneralGemmNumericalPolicyErrorV1>
{
    let mut hasher = Sha256::new();
    hasher.update(BF16_CLOSURE_DOMAIN_V1);
    for bits in 0..=u16::MAX {
        let widened = widen_general_gemm_bf16_v1(bits).to_bits();
        if widened >> 16 != u32::from(bits)
            || widened & 0xffff != 0
            || classify_general_gemm_bf16_v1(bits) != classify_general_gemm_f32_v1(widened)
        {
            return Err(GeneralGemmNumericalPolicyErrorV1::PolicyMutationMismatch);
        }
        hasher.update(bits.to_le_bytes());
        hasher.update(widened.to_le_bytes());
        hasher.update([classify_general_gemm_bf16_v1(bits) as u8]);
    }
    Ok(GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(
        hasher.finalize().into(),
    ))
}

fn check_policy_mutations()
-> Result<GeneralGemmEvidenceIdentityV1, GeneralGemmNumericalPolicyErrorV1> {
    // Sign-dropping widening must be distinguishable even for negative zero.
    let negative_zero = 0x8000_u16;
    let mutated_widen = u32::from(negative_zero & 0x7fff) << 16;
    if widen_general_gemm_bf16_v1(negative_zero).to_bits() == mutated_widen {
        return Err(GeneralGemmNumericalPolicyErrorV1::PolicyMutationMismatch);
    }
    // "Finite" alone is weaker than normal-or-zero because it admits subnormals.
    if classify_general_gemm_bf16_v1(0x0001).is_normal_or_zero() {
        return Err(GeneralGemmNumericalPolicyErrorV1::PolicyMutationMismatch);
    }

    // Exact-real equality cannot select the required FP32 reduction order.
    let source_order = evaluate_general_gemm_numerical_policy_v1(
        &[0x4f80, 0xcf80, 0x3f80],
        &[0x3f80; 3],
        0.0,
        1.0,
        0.0,
    )?;
    let large = widen_general_gemm_bf16_v1(0x4f80);
    let negative_large = widen_general_gemm_bf16_v1(0xcf80);
    let reassociated = separate_f32_add(large, separate_f32_add(negative_large, 1.0));
    if source_order.output_bits == reassociated.to_bits() {
        return Err(GeneralGemmNumericalPolicyErrorV1::PolicyMutationMismatch);
    }

    // Contracting a multiply/add is observably different from two FP32 rounds.
    let epsilon = f32::EPSILON;
    let separate = separate_f32_add(separate_f32_mul(1.0 + epsilon, 1.0 - epsilon), -1.0);
    let contracted = (1.0 + epsilon).mul_add(1.0 - epsilon, -1.0);
    if separate.to_bits() == contracted.to_bits() {
        return Err(GeneralGemmNumericalPolicyErrorV1::PolicyMutationMismatch);
    }

    let epilogue = evaluate_general_gemm_numerical_policy_v1(
        &[0x3f80, 0x4040],
        &[0x4000, 0x4080],
        8.0,
        0.5,
        0.25,
    )?;
    if epilogue.accumulator_bits != 14.0_f32.to_bits() || epilogue.output_bits != 9.0_f32.to_bits()
    {
        return Err(GeneralGemmNumericalPolicyErrorV1::PolicyMutationMismatch);
    }

    let mut hasher = Sha256::new();
    hasher.update(MUTATION_DOMAIN_V1);
    for bits in [
        widen_general_gemm_bf16_v1(negative_zero).to_bits(),
        mutated_widen,
        source_order.output_bits,
        reassociated.to_bits(),
        separate.to_bits(),
        contracted.to_bits(),
        epilogue.accumulator_bits,
        epilogue.output_bits,
    ] {
        hasher.update(bits.to_le_bytes());
    }
    Ok(GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(
        hasher.finalize().into(),
    ))
}

fn require_bf16_class(
    bits: u16,
    stage: GeneralGemmNumericalStageV1,
    index: usize,
) -> Result<(), GeneralGemmNumericalPolicyErrorV1> {
    let class = classify_general_gemm_bf16_v1(bits);
    if !class.is_normal_or_zero() {
        return Err(GeneralGemmNumericalPolicyErrorV1::UnsupportedValue {
            stage,
            index,
            bits: u32::from(bits),
            class,
        });
    }
    Ok(())
}

fn require_f32_class(
    value: f32,
    stage: GeneralGemmNumericalStageV1,
    index: usize,
) -> Result<(), GeneralGemmNumericalPolicyErrorV1> {
    let bits = value.to_bits();
    let class = classify_general_gemm_f32_v1(bits);
    if !class.is_normal_or_zero() {
        return Err(GeneralGemmNumericalPolicyErrorV1::UnsupportedValue {
            stage,
            index,
            bits,
            class,
        });
    }
    Ok(())
}

#[inline(never)]
fn separate_f32_mul(left: f32, right: f32) -> f32 {
    black_box(black_box(left) * black_box(right))
}

#[inline(never)]
fn separate_f32_add(left: f32, right: f32) -> f32 {
    black_box(black_box(left) + black_box(right))
}

const fn part(
    name: &'static str,
    status: GeneralGemmNumericalEvidenceStatusV1,
) -> GeneralGemmNumericalEvidencePartV1 {
    GeneralGemmNumericalEvidencePartV1 { name, status }
}

fn encode_comparison(hasher: &mut Sha256, policy: GeneralGemmNumericalComparisonPolicyV1) {
    match policy {
        GeneralGemmNumericalComparisonPolicyV1::ExactBits => hasher.update([1]),
        GeneralGemmNumericalComparisonPolicyV1::Bounded {
            max_abs_bits,
            max_rel_bits,
            max_ulps,
        } => {
            hasher.update([2]);
            hasher.update(max_abs_bits.to_le_bytes());
            hasher.update(max_rel_bits.to_le_bytes());
            hasher.update(max_ulps.to_le_bytes());
        }
    }
}

const fn ordered_f32(bits: u32) -> u32 {
    if bits & 0x8000_0000 == 0 {
        bits | 0x8000_0000
    } else {
        !bits
    }
}
