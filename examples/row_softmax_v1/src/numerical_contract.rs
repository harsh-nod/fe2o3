//! Executable numerical policy shared by row-softmax source and hardware tests.

/// Largest row admitted by this bounded host oracle.
pub const MAX_ROW_ELEMENTS_V1: usize = 4096;

/// Comparison policy for the reviewed gfx942 OCML exponential profile.
pub const GFX942_OCML_COMPARISON_POLICY_V1: SoftmaxComparisonPolicyV1 = SoftmaxComparisonPolicyV1 {
    absolute_tolerance: 3.0e-6,
    relative_tolerance: 3.0e-5,
    sum_tolerance: 6.0e-5,
    maximum_ulps: 64,
    device_exponential: SoftmaxExponentialV1::OcmlExpF32,
};

/// Exponential implementation used by the profile-neutral host oracle.
pub const HOST_ORACLE_EXPONENTIAL_V1: SoftmaxExponentialV1 = SoftmaxExponentialV1::RustStdF64Exp;

/// Exponential implementation whose error is admitted by a comparison profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoftmaxExponentialV1 {
    /// Rust standard-library `f64::exp`, evaluated on the pinned host toolchain.
    RustStdF64Exp,
    /// The device `__ocml_exp_f32` operation linked from authenticated OCML input.
    OcmlExpF32,
}

/// Fail-closed errors produced before an oracle result is published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoftmaxContractErrorV1 {
    /// A row must contain at least one physical element.
    EmptyRow,
    /// Input, mask, and output lengths must agree.
    LengthMismatch,
    /// The bounded oracle does not admit an arbitrarily large row.
    RowTooLarge,
    /// Every physical input, including a masked input, must be finite.
    NonFiniteInput {
        /// Index of the rejected value.
        index: usize,
    },
    /// An explicit mask must retain at least one active element.
    NoActiveElements,
    /// Stable finite arithmetic unexpectedly produced an invalid state.
    NonFiniteIntermediate,
}

/// Observable state from one deterministic oracle evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSoftmaxOracleStateV1 {
    /// Maximum over active input elements.
    pub maximum: f32,
    /// Sequential active-element sum of stable exponential weights.
    pub denominator: f64,
    /// Number of active elements in the row.
    pub active_elements: usize,
}

/// Explicit policy for comparing a device result with the host oracle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoftmaxComparisonPolicyV1 {
    /// Maximum absolute error accepted for one active output.
    pub absolute_tolerance: f32,
    /// Maximum relative error accepted for one active output.
    pub relative_tolerance: f32,
    /// Maximum absolute error accepted for the active output sum.
    pub sum_tolerance: f32,
    /// Maximum representable-value distance accepted for positive outputs.
    pub maximum_ulps: u32,
    /// Device exponential implementation to which this envelope applies.
    pub device_exponential: SoftmaxExponentialV1,
}

impl SoftmaxComparisonPolicyV1 {
    fn is_valid(self) -> bool {
        self.absolute_tolerance.is_finite()
            && self.absolute_tolerance >= 0.0
            && self.relative_tolerance.is_finite()
            && self.relative_tolerance >= 0.0
            && self.sum_tolerance.is_finite()
            && self.sum_tolerance >= 0.0
    }
}

/// First fail-closed mismatch between a device result and the oracle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SoftmaxComparisonErrorV1 {
    /// Expected, actual, and mask lengths must agree.
    LengthMismatch,
    /// Tolerances must be finite and nonnegative.
    InvalidPolicy,
    /// A device output was NaN, infinite, or negative.
    InvalidOutput {
        /// Index of the invalid output.
        index: usize,
    },
    /// Masked outputs are required to use canonical positive zero exactly.
    MaskedOutputNotPositiveZero {
        /// Index of the masked output.
        index: usize,
    },
    /// An active output exceeded both the numeric and ULP allowances.
    OutputMismatch {
        /// Index of the mismatching output.
        index: usize,
        /// Independently computed oracle result.
        expected: f32,
        /// Observed device result.
        actual: f32,
    },
    /// The active device outputs did not sum to one within policy.
    SumMismatch {
        /// Observed sequential active-output sum.
        actual: f32,
    },
}

fn is_active(mask: Option<&[bool]>, index: usize) -> bool {
    mask.is_none_or(|values| values[index])
}

/// Computes stable row softmax for an optional explicit activity mask.
///
/// The policy rejects every non-finite physical input, even when masked. It
/// converts finite subnormal and signed-zero inputs exactly to `f64`. Maximum
/// subtraction makes every exponential argument
/// nonpositive, so exponential overflow is impossible. Underflow to positive
/// zero is allowed; at least one maximum element contributes `exp(0) == 1`,
/// keeping the denominator positive. Masked outputs are canonical `+0.0`.
/// Results are copied to `output` only after the complete row is validated.
pub fn row_softmax_oracle_v1(
    input: &[f32],
    mask: Option<&[bool]>,
    output: &mut [f32],
) -> Result<RowSoftmaxOracleStateV1, SoftmaxContractErrorV1> {
    if input.is_empty() {
        return Err(SoftmaxContractErrorV1::EmptyRow);
    }
    if input.len() != output.len() || mask.is_some_and(|values| values.len() != input.len()) {
        return Err(SoftmaxContractErrorV1::LengthMismatch);
    }
    if input.len() > MAX_ROW_ELEMENTS_V1 {
        return Err(SoftmaxContractErrorV1::RowTooLarge);
    }
    if let Some((index, _)) = input
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(SoftmaxContractErrorV1::NonFiniteInput { index });
    }

    let active_elements = (0..input.len())
        .filter(|index| is_active(mask, *index))
        .count();
    if active_elements == 0 {
        return Err(SoftmaxContractErrorV1::NoActiveElements);
    }

    let maximum = input
        .iter()
        .enumerate()
        .filter(|(index, _)| is_active(mask, *index))
        .map(|(_, value)| *value)
        .reduce(f32::max)
        .expect("active element count was checked");

    let mut weights = vec![0.0_f64; input.len()];
    let maximum_f64 = f64::from(maximum);
    let mut denominator = 0.0_f64;
    for (index, value) in input.iter().enumerate() {
        if is_active(mask, index) {
            let weight = (f64::from(*value) - maximum_f64).exp();
            if !weight.is_finite() || weight < 0.0 {
                return Err(SoftmaxContractErrorV1::NonFiniteIntermediate);
            }
            weights[index] = weight;
            denominator += weight;
        }
    }
    if !maximum.is_finite() || !denominator.is_finite() || denominator < 1.0 {
        return Err(SoftmaxContractErrorV1::NonFiniteIntermediate);
    }

    let mut result = vec![0.0_f32; input.len()];
    for index in 0..input.len() {
        if is_active(mask, index) {
            result[index] = (weights[index] / denominator) as f32;
            if !result[index].is_finite() || result[index] < 0.0 {
                return Err(SoftmaxContractErrorV1::NonFiniteIntermediate);
            }
        }
    }
    output.copy_from_slice(&result);
    Ok(RowSoftmaxOracleStateV1 {
        maximum,
        denominator,
        active_elements,
    })
}

fn positive_ulp_distance(left: f32, right: f32) -> u32 {
    left.to_bits().abs_diff(right.to_bits())
}

/// Compares a device row with an independently computed oracle row.
///
/// Masked elements use an exact-bit policy. Active elements pass when either
/// their absolute/relative envelope or their positive-value ULP distance is
/// within policy. The active sum is checked independently.
pub fn compare_row_softmax_v1(
    expected: &[f32],
    actual: &[f32],
    mask: Option<&[bool]>,
    policy: SoftmaxComparisonPolicyV1,
) -> Result<(), SoftmaxComparisonErrorV1> {
    if expected.len() != actual.len() || mask.is_some_and(|values| values.len() != expected.len()) {
        return Err(SoftmaxComparisonErrorV1::LengthMismatch);
    }
    if !policy.is_valid() {
        return Err(SoftmaxComparisonErrorV1::InvalidPolicy);
    }

    let mut sum = 0.0_f32;
    for index in 0..expected.len() {
        let observed = actual[index];
        if !observed.is_finite() || observed < 0.0 {
            return Err(SoftmaxComparisonErrorV1::InvalidOutput { index });
        }
        if !is_active(mask, index) {
            if observed.to_bits() != 0.0_f32.to_bits() {
                return Err(SoftmaxComparisonErrorV1::MaskedOutputNotPositiveZero { index });
            }
            continue;
        }

        let reference = expected[index];
        if !reference.is_finite() || reference < 0.0 {
            return Err(SoftmaxComparisonErrorV1::InvalidOutput { index });
        }
        let absolute = (observed - reference).abs();
        let numeric_limit = policy.absolute_tolerance + policy.relative_tolerance * reference.abs();
        if absolute > numeric_limit
            && positive_ulp_distance(observed, reference) > policy.maximum_ulps
        {
            return Err(SoftmaxComparisonErrorV1::OutputMismatch {
                index,
                expected: reference,
                actual: observed,
            });
        }
        sum += observed;
    }
    if !sum.is_finite() || (sum - 1.0).abs() > policy.sum_tolerance {
        return Err(SoftmaxComparisonErrorV1::SumMismatch { actual: sum });
    }
    Ok(())
}
