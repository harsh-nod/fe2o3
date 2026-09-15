//! Transactional FP32 schedule model and independent `f64` differential oracle.

use crate::{
    Bf16ConversionErrorV1, Bf16V1, QWEN3_RMSNORM_EPSILON_BITS_V1, RMSNORM_REDUCTION_STRIDES_V1,
    RMSNORM_WAVE_LANES_V1, ValidatedRmsNormProfileV1,
};

/// Logical input or output buffer named by a reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RmsNormBufferV1 {
    /// BF16 activation input.
    Activation,
    /// BF16 residual input.
    ResidualInput,
    /// Shared BF16 RMSNorm weight.
    Weight,
    /// BF16 normalized output.
    NormalizedOutput,
    /// BF16 residual-sum output.
    ResidualOutput,
}

/// FP32 stage that produced a non-finite intermediate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RmsNormArithmeticStageV1 {
    /// Residual addition.
    ResidualAdd,
    /// Square multiplication or lane accumulation.
    SquareAccumulation,
    /// Fixed Wave64 reduction tree.
    WaveReduction,
    /// Mean, epsilon, square root, or reciprocal.
    ReciprocalRoot,
    /// Normalized scale and weight multiplication.
    OutputScale,
    /// BF16 output conversion.
    OutputCast,
}

/// Fail-closed host reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RmsNormReferenceErrorV1 {
    /// One buffer length differed from the exact profile extent.
    WrongLength {
        /// Buffer whose length differed.
        buffer: RmsNormBufferV1,
        /// Required element count.
        expected: usize,
        /// Observed element count.
        actual: usize,
    },
    /// A physical BF16 input was NaN or infinite.
    NonFiniteInput {
        /// Input buffer.
        buffer: RmsNormBufferV1,
        /// Failing element.
        index: usize,
    },
    /// FP32 evaluation produced NaN or infinity.
    NonFiniteIntermediate {
        /// Flattened row.
        row: usize,
        /// Failing stage.
        stage: RmsNormArithmeticStageV1,
    },
    /// A bounded transactional scratch allocation failed.
    AllocationFailure,
}

/// Summary of a complete FP32 schedule-model evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RmsNormReferenceStateV1 {
    /// Number of completely evaluated rows.
    pub rows: usize,
    /// Smallest per-row reciprocal RMS.
    pub minimum_reciprocal_rms: f32,
    /// Largest per-row reciprocal RMS.
    pub maximum_reciprocal_rms: f32,
}

/// Independently evaluated idealized `f64` results for differential testing.
#[derive(Clone, Debug, PartialEq)]
pub struct RmsNormF64OracleV1 {
    /// Idealized residual sums, evaluated with host `f64` operations.
    pub residual_sum: Vec<f64>,
    /// Idealized normalized and weighted outputs.
    pub normalized: Vec<f64>,
}

fn allocate_filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, RmsNormReferenceErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| RmsNormReferenceErrorV1::AllocationFailure)?;
    result.resize(length, value);
    Ok(result)
}

fn check_length(
    buffer: RmsNormBufferV1,
    actual: usize,
    expected: usize,
) -> Result<(), RmsNormReferenceErrorV1> {
    if actual != expected {
        return Err(RmsNormReferenceErrorV1::WrongLength {
            buffer,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_finite_input(
    buffer: RmsNormBufferV1,
    values: &[Bf16V1],
) -> Result<(), RmsNormReferenceErrorV1> {
    if let Some((index, _)) = values
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(RmsNormReferenceErrorV1::NonFiniteInput { buffer, index });
    }
    Ok(())
}

fn cast_output(value: f32, row: usize) -> Result<Bf16V1, RmsNormReferenceErrorV1> {
    Bf16V1::from_f32_rne(value).map_err(|error| match error {
        Bf16ConversionErrorV1::NonFiniteInput | Bf16ConversionErrorV1::NonFiniteOutput => {
            RmsNormReferenceErrorV1::NonFiniteIntermediate {
                row,
                stage: RmsNormArithmeticStageV1::OutputCast,
            }
        }
    })
}

fn validate_buffers(
    profile: ValidatedRmsNormProfileV1,
    activation: &[Bf16V1],
    residual: &[Bf16V1],
    weight: &[Bf16V1],
    normalized_output: &[Bf16V1],
    residual_output: &[Bf16V1],
) -> Result<(), RmsNormReferenceErrorV1> {
    let descriptor = profile.descriptor();
    let elements = profile.resources().activation_elements;
    check_length(RmsNormBufferV1::Activation, activation.len(), elements)?;
    check_length(RmsNormBufferV1::ResidualInput, residual.len(), elements)?;
    check_length(
        RmsNormBufferV1::Weight,
        weight.len(),
        descriptor.hidden_size,
    )?;
    check_length(
        RmsNormBufferV1::NormalizedOutput,
        normalized_output.len(),
        elements,
    )?;
    check_length(
        RmsNormBufferV1::ResidualOutput,
        residual_output.len(),
        elements,
    )?;
    check_finite_input(RmsNormBufferV1::Activation, activation)?;
    check_finite_input(RmsNormBufferV1::ResidualInput, residual)?;
    check_finite_input(RmsNormBufferV1::Weight, weight)?;
    Ok(())
}

/// Executes the exact Wave64 FP32 evaluation order into temporary outputs.
///
/// For each row, lane `l` visits columns `l, l + 64, ...` in ascending order.
/// It adds decoded BF16 activation and residual once in FP32, multiplies the
/// sum by itself in FP32, and accumulates that lane's terms in FP32. The 64
/// partials reduce in stages `[32, 16, 8, 4, 2, 1]`; each stage evaluates
/// participating lanes in ascending order. The mean, epsilon add, `sqrt`, and
/// reciprocal are FP32 operations. Each normalized output is evaluated as
/// `(residual_sum * reciprocal_rms) * weight` and both outputs are converted
/// by BF16 round-to-nearest, ties-to-even.
///
/// This is a pinned host schedule model, not evidence that Rust, LLVM, OCML,
/// or gfx942 uses or refines these operations. Outputs are unchanged on every
/// error and are copied only after all rows succeed.
pub fn rmsnorm_residual_reference_v1(
    profile: ValidatedRmsNormProfileV1,
    activation: &[Bf16V1],
    residual: &[Bf16V1],
    weight: &[Bf16V1],
    normalized_output: &mut [Bf16V1],
    residual_output: &mut [Bf16V1],
) -> Result<RmsNormReferenceStateV1, RmsNormReferenceErrorV1> {
    validate_buffers(
        profile,
        activation,
        residual,
        weight,
        normalized_output,
        residual_output,
    )?;
    let descriptor = profile.descriptor();
    let elements = profile.resources().activation_elements;
    let mut normalized_result = allocate_filled(elements, Bf16V1::default())?;
    let mut residual_result = allocate_filled(elements, Bf16V1::default())?;
    let mut row_sums = allocate_filled(descriptor.hidden_size, 0.0_f32)?;
    let epsilon = f32::from_bits(QWEN3_RMSNORM_EPSILON_BITS_V1);
    let mut minimum_reciprocal_rms = f32::INFINITY;
    let mut maximum_reciprocal_rms = 0.0_f32;

    for row in 0..descriptor.rows {
        let row_base = row * descriptor.hidden_size;
        let mut lane_partials = [0.0_f32; RMSNORM_WAVE_LANES_V1];
        for (lane, partial) in lane_partials.iter_mut().enumerate() {
            let mut column = lane;
            while column < descriptor.hidden_size {
                let index = row_base + column;
                let sum = activation[index].to_f32() + residual[index].to_f32();
                if !sum.is_finite() {
                    return Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
                        row,
                        stage: RmsNormArithmeticStageV1::ResidualAdd,
                    });
                }
                row_sums[column] = sum;
                let square = sum * sum;
                *partial += square;
                if !square.is_finite() || !partial.is_finite() {
                    return Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
                        row,
                        stage: RmsNormArithmeticStageV1::SquareAccumulation,
                    });
                }
                column += RMSNORM_WAVE_LANES_V1;
            }
        }
        for stride in RMSNORM_REDUCTION_STRIDES_V1 {
            for lane in 0..usize::from(stride) {
                lane_partials[lane] += lane_partials[lane + usize::from(stride)];
                if !lane_partials[lane].is_finite() {
                    return Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
                        row,
                        stage: RmsNormArithmeticStageV1::WaveReduction,
                    });
                }
            }
        }
        let mean_square = lane_partials[0] / descriptor.hidden_size as f32;
        let shifted = mean_square + epsilon;
        let reciprocal_rms = 1.0_f32 / shifted.sqrt();
        if !mean_square.is_finite()
            || !shifted.is_finite()
            || !reciprocal_rms.is_finite()
            || reciprocal_rms <= 0.0
        {
            return Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
                row,
                stage: RmsNormArithmeticStageV1::ReciprocalRoot,
            });
        }
        minimum_reciprocal_rms = minimum_reciprocal_rms.min(reciprocal_rms);
        maximum_reciprocal_rms = maximum_reciprocal_rms.max(reciprocal_rms);

        for lane in 0..RMSNORM_WAVE_LANES_V1 {
            let mut column = lane;
            while column < descriptor.hidden_size {
                let index = row_base + column;
                let scaled = row_sums[column] * reciprocal_rms;
                let normalized = scaled * weight[column].to_f32();
                if !scaled.is_finite() || !normalized.is_finite() {
                    return Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
                        row,
                        stage: RmsNormArithmeticStageV1::OutputScale,
                    });
                }
                residual_result[index] = cast_output(row_sums[column], row)?;
                normalized_result[index] = cast_output(normalized, row)?;
                column += RMSNORM_WAVE_LANES_V1;
            }
        }
    }

    normalized_output.copy_from_slice(&normalized_result);
    residual_output.copy_from_slice(&residual_result);
    Ok(RmsNormReferenceStateV1 {
        rows: descriptor.rows,
        minimum_reciprocal_rms,
        maximum_reciprocal_rms,
    })
}

/// Computes an independent idealized host `f64` oracle.
///
/// The mathematical relation per row is `z_i = x_i + residual_i`,
/// `r = 1 / sqrt(sum(z_i^2) / hidden + epsilon)`, and
/// `normalized_i = z_i * r * weight_i`. This function evaluates that relation
/// sequentially in host `f64`; it deliberately does not reproduce the Wave64
/// FP32 reduction or BF16 output cast. It is a differential test oracle, not a
/// proof of real arithmetic, IEEE-754 behavior, or target instructions.
pub fn rmsnorm_residual_f64_oracle_v1(
    profile: ValidatedRmsNormProfileV1,
    activation: &[Bf16V1],
    residual: &[Bf16V1],
    weight: &[Bf16V1],
) -> Result<RmsNormF64OracleV1, RmsNormReferenceErrorV1> {
    let elements = profile.resources().activation_elements;
    let placeholder = allocate_filled(elements, Bf16V1::default())?;
    validate_buffers(
        profile,
        activation,
        residual,
        weight,
        &placeholder,
        &placeholder,
    )?;
    let descriptor = profile.descriptor();
    let mut residual_sum = allocate_filled(elements, 0.0_f64)?;
    let mut normalized = allocate_filled(elements, 0.0_f64)?;
    let epsilon = f64::from(f32::from_bits(QWEN3_RMSNORM_EPSILON_BITS_V1));

    for row in 0..descriptor.rows {
        let row_base = row * descriptor.hidden_size;
        let mut sum_of_squares = 0.0_f64;
        for column in 0..descriptor.hidden_size {
            let index = row_base + column;
            let sum = f64::from(activation[index].to_f32()) + f64::from(residual[index].to_f32());
            residual_sum[index] = sum;
            sum_of_squares += sum * sum;
        }
        let reciprocal_rms =
            1.0_f64 / (sum_of_squares / descriptor.hidden_size as f64 + epsilon).sqrt();
        if !reciprocal_rms.is_finite() || reciprocal_rms <= 0.0 {
            return Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
                row,
                stage: RmsNormArithmeticStageV1::ReciprocalRoot,
            });
        }
        for (column, weight_value) in weight.iter().enumerate() {
            let index = row_base + column;
            normalized[index] =
                residual_sum[index] * reciprocal_rms * f64::from(weight_value.to_f32());
            if !normalized[index].is_finite() {
                return Err(RmsNormReferenceErrorV1::NonFiniteIntermediate {
                    row,
                    stage: RmsNormArithmeticStageV1::OutputScale,
                });
            }
        }
    }

    Ok(RmsNormF64OracleV1 {
        residual_sum,
        normalized,
    })
}
