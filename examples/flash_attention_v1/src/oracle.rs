//! Independent CPU reference and fail-closed arithmetic preflight.

use crate::contract::{
    ATTENTION_SCALE_BITS_V1, FLASH_ATTENTION_HEAD_DIMENSION_V1, FLASH_ATTENTION_INPUT_ELEMENTS_V1,
    FLASH_ATTENTION_SEQUENCE_LENGTH_V1, TensorV1,
};

/// Arithmetic stage that violated the finite strict-FP32 profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticStageV1 {
    /// One FP32 Q*K product.
    DotProduct,
    /// Sequential FP32 dot-product accumulation.
    DotSum,
    /// FP32 multiplication by the exact 0.25 attention scale.
    ScaledScore,
    /// Host exponential surrogate for the online rescaling weights.
    OnlineExponential,
    /// FP32 online denominator update.
    OnlineDenominator,
    /// FP32 online V numerator update.
    OnlineNumerator,
    /// FP32 division of the online numerator by its denominator.
    OnlineOutput,
    /// Independent FP64 reference result could not be represented as finite FP32.
    ReferenceOutput,
}

/// Fail-closed CPU-oracle rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionOracleErrorV1 {
    /// A tensor length did not match the exact fixed profile.
    WrongLength {
        /// Tensor whose extent was wrong.
        tensor: TensorV1,
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
    /// An input contained NaN or infinity.
    NonFiniteInput {
        /// Input tensor containing the value.
        tensor: TensorV1,
        /// Contiguous element index.
        index: usize,
    },
    /// A strict-FP32 profile intermediate was invalid.
    NonFiniteIntermediate {
        /// Query row being evaluated.
        query_row: usize,
        /// Participating key row.
        key_row: usize,
        /// Output column for V accumulation, when applicable.
        output_column: Option<usize>,
        /// Rejected arithmetic stage.
        stage: ArithmeticStageV1,
    },
}

/// Observable state from the independent two-pass FP64 reference.
#[derive(Clone, Debug, PartialEq)]
pub struct FlashAttentionOracleStateV1 {
    /// Per-query maximum over causal FP64 scores.
    pub row_maxima: [f64; FLASH_ATTENTION_SEQUENCE_LENGTH_V1],
    /// Per-query sum of stable FP64 exponential weights.
    pub row_denominators: [f64; FLASH_ATTENTION_SEQUENCE_LENGTH_V1],
}

fn validate_length(tensor: TensorV1, actual: usize) -> Result<(), FlashAttentionOracleErrorV1> {
    if actual == FLASH_ATTENTION_INPUT_ELEMENTS_V1 {
        Ok(())
    } else {
        Err(FlashAttentionOracleErrorV1::WrongLength {
            tensor,
            expected: FLASH_ATTENTION_INPUT_ELEMENTS_V1,
            actual,
        })
    }
}

fn validate_finite(tensor: TensorV1, values: &[f32]) -> Result<(), FlashAttentionOracleErrorV1> {
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(FlashAttentionOracleErrorV1::NonFiniteInput { tensor, index });
        }
    }
    Ok(())
}

fn strict_f32_scores(
    q: &[f32],
    k: &[f32],
) -> Result<
    [[f32; FLASH_ATTENTION_SEQUENCE_LENGTH_V1]; FLASH_ATTENTION_SEQUENCE_LENGTH_V1],
    FlashAttentionOracleErrorV1,
> {
    let mut scores =
        [[0.0_f32; FLASH_ATTENTION_SEQUENCE_LENGTH_V1]; FLASH_ATTENTION_SEQUENCE_LENGTH_V1];
    for (query_row, score_row) in scores.iter_mut().enumerate() {
        for (key_row, score_slot) in score_row.iter_mut().enumerate().take(query_row + 1) {
            let mut dot = 0.0_f32;
            for feature in 0..FLASH_ATTENTION_HEAD_DIMENSION_V1 {
                let q_index = query_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature;
                let k_index = key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature;
                let product = q[q_index] * k[k_index];
                if !product.is_finite() {
                    return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                        query_row,
                        key_row,
                        output_column: None,
                        stage: ArithmeticStageV1::DotProduct,
                    });
                }
                dot += product;
                if !dot.is_finite() {
                    return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                        query_row,
                        key_row,
                        output_column: None,
                        stage: ArithmeticStageV1::DotSum,
                    });
                }
            }
            let score = dot * f32::from_bits(ATTENTION_SCALE_BITS_V1);
            if !score.is_finite() {
                return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                    query_row,
                    key_row,
                    output_column: None,
                    stage: ArithmeticStageV1::ScaledScore,
                });
            }
            *score_slot = score;
        }
    }
    Ok(scores)
}

fn preflight_online_f32(
    scores: &[[f32; FLASH_ATTENTION_SEQUENCE_LENGTH_V1]; FLASH_ATTENTION_SEQUENCE_LENGTH_V1],
    v: &[f32],
) -> Result<(), FlashAttentionOracleErrorV1> {
    for (query_row, score_row) in scores.iter().enumerate() {
        for output_column in 0..FLASH_ATTENTION_HEAD_DIMENSION_V1 {
            let mut running_max = 0.0_f32;
            let mut running_sum = 0.0_f32;
            let mut numerator = 0.0_f32;
            for key_row in 0..=query_row {
                let score = score_row[key_row];
                let value = v[key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + output_column];
                if key_row == 0 {
                    running_max = score;
                    running_sum = 1.0;
                    numerator = value;
                    continue;
                }

                let next_max = running_max.max(score);
                let previous_weight = (running_max - next_max).exp();
                let current_weight = (score - next_max).exp();
                if !previous_weight.is_finite() || !current_weight.is_finite() {
                    return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                        query_row,
                        key_row,
                        output_column: Some(output_column),
                        stage: ArithmeticStageV1::OnlineExponential,
                    });
                }

                running_sum = running_sum * previous_weight + current_weight;
                if !running_sum.is_finite() || running_sum <= 0.0 {
                    return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                        query_row,
                        key_row,
                        output_column: Some(output_column),
                        stage: ArithmeticStageV1::OnlineDenominator,
                    });
                }
                numerator = numerator * previous_weight + value * current_weight;
                if !numerator.is_finite() {
                    return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                        query_row,
                        key_row,
                        output_column: Some(output_column),
                        stage: ArithmeticStageV1::OnlineNumerator,
                    });
                }
                running_max = next_max;
            }
            if !(numerator / running_sum).is_finite() {
                return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                    query_row,
                    key_row: query_row,
                    output_column: Some(output_column),
                    stage: ArithmeticStageV1::OnlineOutput,
                });
            }
        }
    }
    Ok(())
}

fn reference_score(q: &[f32], k: &[f32], query_row: usize, key_row: usize) -> f64 {
    let mut dot = 0.0_f64;
    for feature in 0..FLASH_ATTENTION_HEAD_DIMENSION_V1 {
        let q_index = query_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature;
        let k_index = key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature;
        dot += f64::from(q[q_index]) * f64::from(k[k_index]);
    }
    dot * 0.25_f64
}

/// Computes an independent two-pass stable causal attention reference.
///
/// The output is committed only after all shape, finite-input, strict-FP32
/// preflight, and FP64 reference checks succeed. The reference deliberately
/// does not call or duplicate the kernel's online recurrence: it materializes
/// each bounded causal score row, then performs a separate maximum pass and a
/// stable weighted-value pass in FP64. `f32::exp` in the preflight is a host
/// surrogate, not OCML evidence.
pub fn flash_attention_oracle_v1(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    output: &mut [f32],
) -> Result<FlashAttentionOracleStateV1, FlashAttentionOracleErrorV1> {
    validate_length(TensorV1::Q, q.len())?;
    validate_length(TensorV1::K, k.len())?;
    validate_length(TensorV1::V, v.len())?;
    validate_length(TensorV1::O, output.len())?;
    validate_finite(TensorV1::Q, q)?;
    validate_finite(TensorV1::K, k)?;
    validate_finite(TensorV1::V, v)?;

    let strict_scores = strict_f32_scores(q, k)?;
    preflight_online_f32(&strict_scores, v)?;

    let mut staged = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut row_maxima = [0.0_f64; FLASH_ATTENTION_SEQUENCE_LENGTH_V1];
    let mut row_denominators = [0.0_f64; FLASH_ATTENTION_SEQUENCE_LENGTH_V1];
    for query_row in 0..FLASH_ATTENTION_SEQUENCE_LENGTH_V1 {
        let mut scores = [0.0_f64; FLASH_ATTENTION_SEQUENCE_LENGTH_V1];
        for (key_row, score) in scores.iter_mut().enumerate().take(query_row + 1) {
            *score = reference_score(q, k, query_row, key_row);
        }
        let mut maximum = scores[0];
        for score in &scores[1..=query_row] {
            maximum = maximum.max(*score);
        }
        let mut weights = [0.0_f64; FLASH_ATTENTION_SEQUENCE_LENGTH_V1];
        let mut denominator = 0.0_f64;
        for (key_row, weight) in weights.iter_mut().enumerate().take(query_row + 1) {
            *weight = (scores[key_row] - maximum).exp();
            denominator += *weight;
        }

        row_maxima[query_row] = maximum;
        row_denominators[query_row] = denominator;
        for output_column in 0..FLASH_ATTENTION_HEAD_DIMENSION_V1 {
            let mut numerator = 0.0_f64;
            for (key_row, weight) in weights.iter().enumerate().take(query_row + 1) {
                let value_index = key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + output_column;
                numerator += *weight * f64::from(v[value_index]);
            }
            let value = (numerator / denominator) as f32;
            if !value.is_finite() {
                return Err(FlashAttentionOracleErrorV1::NonFiniteIntermediate {
                    query_row,
                    key_row: query_row,
                    output_column: Some(output_column),
                    stage: ArithmeticStageV1::ReferenceOutput,
                });
            }
            staged[query_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + output_column] = value;
        }
    }

    output.copy_from_slice(&staged);
    Ok(FlashAttentionOracleStateV1 {
        row_maxima,
        row_denominators,
    })
}
