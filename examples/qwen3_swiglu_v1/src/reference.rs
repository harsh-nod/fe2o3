//! Transactional FP32 schedule model and independent `f64` oracle.

use crate::{Bf16ConversionErrorV1, Bf16V1, ValidatedSwiGluCandidateV1};

/// Logical buffer named by a reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwiGluBufferV1 {
    /// BF16 gate-projection output.
    Gate,
    /// BF16 up-projection output.
    Up,
    /// BF16 activated output consumed by down projection.
    Activated,
}

/// FP32 stage that produced a non-finite intermediate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwiGluArithmeticStageV1 {
    /// Stable exponential and sigmoid evaluation.
    Sigmoid,
    /// Gate multiplied by sigmoid.
    Silu,
    /// SiLU multiplied by up-projection value.
    Product,
    /// BF16 round-to-nearest, ties-to-even conversion.
    OutputCast,
}

/// Fail-closed host reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwiGluReferenceErrorV1 {
    /// One buffer length differed from the exact profile extent.
    WrongLength {
        /// Buffer whose length differed.
        buffer: SwiGluBufferV1,
        /// Required element count.
        expected: usize,
        /// Observed element count.
        actual: usize,
    },
    /// One physical BF16 input was NaN or infinite.
    NonFiniteInput {
        /// Input buffer.
        buffer: SwiGluBufferV1,
        /// Failing element.
        index: usize,
    },
    /// FP32 evaluation produced NaN or infinity.
    NonFiniteIntermediate {
        /// Failing element.
        index: usize,
        /// Failing arithmetic stage.
        stage: SwiGluArithmeticStageV1,
    },
    /// A bounded transactional allocation failed.
    AllocationFailure,
}

/// Summary of a complete schedule-model evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwiGluReferenceStateV1 {
    /// Exact number of output elements published.
    pub elements: usize,
    /// Smallest finite FP32 product before BF16 conversion.
    pub minimum_product: f32,
    /// Largest finite FP32 product before BF16 conversion.
    pub maximum_product: f32,
}

/// Independently evaluated idealized `f64` output.
#[derive(Clone, Debug, PartialEq)]
pub struct SwiGluF64OracleV1 {
    /// Exact elementwise outputs before BF16 conversion.
    pub activated: Vec<f64>,
}

fn check_length(
    buffer: SwiGluBufferV1,
    actual: usize,
    expected: usize,
) -> Result<(), SwiGluReferenceErrorV1> {
    if actual != expected {
        return Err(SwiGluReferenceErrorV1::WrongLength {
            buffer,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_finite_input(
    buffer: SwiGluBufferV1,
    values: &[Bf16V1],
) -> Result<(), SwiGluReferenceErrorV1> {
    if let Some((index, _)) = values
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(SwiGluReferenceErrorV1::NonFiniteInput { buffer, index });
    }
    Ok(())
}

fn allocate_output(length: usize) -> Result<Vec<Bf16V1>, SwiGluReferenceErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| SwiGluReferenceErrorV1::AllocationFailure)?;
    result.resize(length, Bf16V1::default());
    Ok(result)
}

fn stable_sigmoid_f32(gate: f32) -> f32 {
    if gate >= 0.0 {
        1.0 / (1.0 + (-gate).exp())
    } else {
        let exponential = gate.exp();
        exponential / (1.0 + exponential)
    }
}

fn stable_sigmoid_f64(gate: f64) -> f64 {
    if gate >= 0.0 {
        1.0 / (1.0 + (-gate).exp())
    } else {
        let exponential = gate.exp();
        exponential / (1.0 + exponential)
    }
}

fn cast_output(value: f32, index: usize) -> Result<Bf16V1, SwiGluReferenceErrorV1> {
    Bf16V1::from_f32_rne(value).map_err(|error| match error {
        Bf16ConversionErrorV1::NonFiniteInput | Bf16ConversionErrorV1::NonFiniteOutput => {
            SwiGluReferenceErrorV1::NonFiniteIntermediate {
                index,
                stage: SwiGluArithmeticStageV1::OutputCast,
            }
        }
    })
}

/// Executes the exact stable FP32 SwiGLU order into transactional scratch.
///
/// Each element decodes gate and up from BF16. For nonnegative gate it uses
/// `1 / (1 + exp(-gate))`; for negative gate it uses
/// `exp(gate) / (1 + exp(gate))`. It then evaluates `gate * sigmoid`, followed
/// by `silu * up`, and rounds once to BF16 using round-to-nearest, ties-to-even.
/// The destination is unchanged on every error and is copied only after the
/// complete invocation succeeds.
///
/// This host model is not evidence that Rust, LLVM, OCML, or gfx942 uses or
/// refines these operations.
pub fn swiglu_reference_v1(
    candidate: ValidatedSwiGluCandidateV1,
    gate: &[Bf16V1],
    up: &[Bf16V1],
    activated: &mut [Bf16V1],
) -> Result<SwiGluReferenceStateV1, SwiGluReferenceErrorV1> {
    let elements = candidate.profile().resources().elements;
    check_length(SwiGluBufferV1::Gate, gate.len(), elements)?;
    check_length(SwiGluBufferV1::Up, up.len(), elements)?;
    check_length(SwiGluBufferV1::Activated, activated.len(), elements)?;
    check_finite_input(SwiGluBufferV1::Gate, gate)?;
    check_finite_input(SwiGluBufferV1::Up, up)?;

    let mut result = allocate_output(elements)?;
    let mut minimum_product = f32::INFINITY;
    let mut maximum_product = f32::NEG_INFINITY;
    for index in 0..elements {
        let gate_value = gate[index].to_f32();
        let sigmoid = stable_sigmoid_f32(gate_value);
        if !sigmoid.is_finite() || !(0.0..=1.0).contains(&sigmoid) {
            return Err(SwiGluReferenceErrorV1::NonFiniteIntermediate {
                index,
                stage: SwiGluArithmeticStageV1::Sigmoid,
            });
        }
        let silu = gate_value * sigmoid;
        if !silu.is_finite() {
            return Err(SwiGluReferenceErrorV1::NonFiniteIntermediate {
                index,
                stage: SwiGluArithmeticStageV1::Silu,
            });
        }
        let product = silu * up[index].to_f32();
        if !product.is_finite() {
            return Err(SwiGluReferenceErrorV1::NonFiniteIntermediate {
                index,
                stage: SwiGluArithmeticStageV1::Product,
            });
        }
        result[index] = cast_output(product, index)?;
        minimum_product = minimum_product.min(product);
        maximum_product = maximum_product.max(product);
    }
    activated.copy_from_slice(&result);
    Ok(SwiGluReferenceStateV1 {
        elements,
        minimum_product,
        maximum_product,
    })
}

/// Evaluates the same piecewise mathematical expression using host `f64`.
///
/// This oracle is independent of the FP32 schedule model and is intended only
/// for differential tests. It is not a real-number or machine refinement.
pub fn swiglu_f64_oracle_v1(
    candidate: ValidatedSwiGluCandidateV1,
    gate: &[Bf16V1],
    up: &[Bf16V1],
) -> Result<SwiGluF64OracleV1, SwiGluReferenceErrorV1> {
    let elements = candidate.profile().resources().elements;
    check_length(SwiGluBufferV1::Gate, gate.len(), elements)?;
    check_length(SwiGluBufferV1::Up, up.len(), elements)?;
    check_finite_input(SwiGluBufferV1::Gate, gate)?;
    check_finite_input(SwiGluBufferV1::Up, up)?;
    let mut activated = Vec::new();
    activated
        .try_reserve_exact(elements)
        .map_err(|_| SwiGluReferenceErrorV1::AllocationFailure)?;
    for index in 0..elements {
        let gate_value = f64::from(gate[index].to_f32());
        let up_value = f64::from(up[index].to_f32());
        let product = (gate_value * stable_sigmoid_f64(gate_value)) * up_value;
        if !product.is_finite() {
            return Err(SwiGluReferenceErrorV1::NonFiniteIntermediate {
                index,
                stage: SwiGluArithmeticStageV1::Product,
            });
        }
        activated.push(product);
    }
    Ok(SwiGluF64OracleV1 { activated })
}
