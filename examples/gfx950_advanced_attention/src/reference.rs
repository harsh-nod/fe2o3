//! Independent safe CPU references for the fixed teaching profiles.

use crate::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, HEAD_DIMENSION_V1, KDA_TAPS_V1,
    KIMI_K3_DECODE_STATE_ELEMENTS_V1, KIMI_K3_GATE_LOWER_BOUND_V1, KIMI_K3_HEAD_DIMENSION_V1,
    KIMI_K3_KDA_ATTENTION_SCALE_V1, KIMI_K3_QK_NORM_EPSILON_V1, KIMI_K3_VALUE_DIMENSION_V1,
    MIXING_STREAMS_V1, PREFILL_TOKENS_V1, SELECTED_BLOCKS_V1, SELECTED_TOKENS_V1,
    SINKHORN_ITERATIONS_V1, TOKENS_PER_BLOCK_V1,
};

const ATTENTION_SCALE_V1: f32 = 0.088_388_346;
const RMS_EPSILON_V1: f32 = 1.0e-5;

/// Input-policy failures reported by the safe CPU references.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceErrorV1 {
    /// One or more slices do not match the exact fixed profile.
    Shape,
    /// An input or computed value is NaN or infinite.
    NonFinite,
    /// A normalization denominator is not finite and strictly positive.
    DegenerateNormalization,
}

/// State and normalized output produced by one decode step.
#[derive(Clone, Debug, PartialEq)]
pub struct KdaDecodeOutputV1 {
    /// Updated recurrent state.
    pub state: Vec<f32>,
    /// RMS-normalized updated state.
    pub normalized: Vec<f32>,
}

/// Final state and all token outputs produced by prefill.
#[derive(Clone, Debug, PartialEq)]
pub struct KdaPrefillOutputV1 {
    /// State after token seven.
    pub final_state: Vec<f32>,
    /// Token-major normalized outputs with shape `[8][16]`.
    pub normalized: Vec<f32>,
}

/// Core Kimi K3 KDA decode output for one sequence, value head, and token.
#[derive(Clone, Debug, PartialEq)]
pub struct KimiK3KdaDecodeOutputV1 {
    /// Updated recurrent state in V-first `[128][128]` row-major layout.
    pub final_state: Vec<f32>,
    /// Core KDA output before the model's output RMS gate and output projection.
    pub output: Vec<f32>,
}

/// Selected token IDs and output from indexed sparse attention.
#[derive(Clone, Debug, PartialEq)]
pub struct SparseAttentionOutputV1 {
    /// Three unique tokens selected from the two highest-scoring blocks.
    pub selected: [u32; SELECTED_TOKENS_V1],
    /// One 16-channel attention output.
    pub output: Vec<f32>,
}

fn validate_finite_v1(values: &[f32], expected: usize) -> Result<(), ReferenceErrorV1> {
    if values.len() != expected {
        return Err(ReferenceErrorV1::Shape);
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(ReferenceErrorV1::NonFinite);
    }
    Ok(())
}

fn sigmoid_reference_v1(value: f32) -> Result<f32, ReferenceErrorV1> {
    let result = 1.0 / (1.0 + (-value).exp());
    result
        .is_finite()
        .then_some(result)
        .ok_or(ReferenceErrorV1::NonFinite)
}

fn l2_norm_reference_v1(values: &[f32], expected: usize) -> Result<f32, ReferenceErrorV1> {
    validate_finite_v1(values, expected)?;
    let square_sum = values.iter().map(|value| value * value).sum::<f32>();
    let norm = (square_sum + KIMI_K3_QK_NORM_EPSILON_V1).sqrt();
    if !norm.is_finite() || norm <= 0.0 {
        return Err(ReferenceErrorV1::DegenerateNormalization);
    }
    Ok(norm)
}

fn kimi_k3_log_decay_reference_v1(
    gate: f32,
    a_log: f32,
    dt_bias: f32,
) -> Result<f32, ReferenceErrorV1> {
    let result =
        KIMI_K3_GATE_LOWER_BOUND_V1 * sigmoid_reference_v1(a_log.exp() * (gate + dt_bias))?;
    result
        .is_finite()
        .then_some(result)
        .ok_or(ReferenceErrorV1::NonFinite)
}

/// Evaluates one single-head Kimi K3 fused-recurrent KDA core decode step.
///
/// This mirrors the FLA decode contract used by Kimi K3 for the core KDA op:
/// q/k L2 normalization, beta sigmoid, default `1 / sqrt(K)` scale, safe-gate
/// log decay with `lower_bound = -5`, and V-first state layout. The model's
/// short convolution, output RMS gate, output projection, and multi-head/cache
/// batching are outside this reference.
pub fn kimi_k3_kda_decode_reference_v1(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    gate: &[f32],
    beta_logit: &[f32],
    a_log: &[f32],
    dt_bias: &[f32],
    initial_state: &[f32],
) -> Result<KimiK3KdaDecodeOutputV1, ReferenceErrorV1> {
    validate_finite_v1(q, KIMI_K3_HEAD_DIMENSION_V1)?;
    validate_finite_v1(k, KIMI_K3_HEAD_DIMENSION_V1)?;
    validate_finite_v1(v, KIMI_K3_VALUE_DIMENSION_V1)?;
    validate_finite_v1(gate, KIMI_K3_HEAD_DIMENSION_V1)?;
    validate_finite_v1(beta_logit, 1)?;
    validate_finite_v1(a_log, 1)?;
    validate_finite_v1(dt_bias, KIMI_K3_HEAD_DIMENSION_V1)?;
    validate_finite_v1(initial_state, KIMI_K3_DECODE_STATE_ELEMENTS_V1)?;

    let q_norm = l2_norm_reference_v1(q, KIMI_K3_HEAD_DIMENSION_V1)?;
    let k_norm = l2_norm_reference_v1(k, KIMI_K3_HEAD_DIMENSION_V1)?;
    let beta = sigmoid_reference_v1(beta_logit[0])?;
    let mut retention = vec![0.0_f32; KIMI_K3_HEAD_DIMENSION_V1];
    let mut q_scaled = vec![0.0_f32; KIMI_K3_HEAD_DIMENSION_V1];
    let mut k_normalized = vec![0.0_f32; KIMI_K3_HEAD_DIMENSION_V1];
    for key in 0..KIMI_K3_HEAD_DIMENSION_V1 {
        let log_decay = kimi_k3_log_decay_reference_v1(gate[key], a_log[0], dt_bias[key])?;
        retention[key] = log_decay.exp();
        q_scaled[key] = q[key] / q_norm * KIMI_K3_KDA_ATTENTION_SCALE_V1;
        k_normalized[key] = k[key] / k_norm;
    }

    let mut final_state = vec![0.0_f32; KIMI_K3_DECODE_STATE_ELEMENTS_V1];
    let mut output = vec![0.0_f32; KIMI_K3_VALUE_DIMENSION_V1];
    for value in 0..KIMI_K3_VALUE_DIMENSION_V1 {
        let row_base = value * KIMI_K3_HEAD_DIMENSION_V1;
        let mut projection = 0.0_f32;
        for key in 0..KIMI_K3_HEAD_DIMENSION_V1 {
            projection += initial_state[row_base + key] * retention[key] * k_normalized[key];
        }
        let correction = beta * (v[value] - projection);
        for key in 0..KIMI_K3_HEAD_DIMENSION_V1 {
            let updated =
                initial_state[row_base + key] * retention[key] + correction * k_normalized[key];
            if !updated.is_finite() {
                return Err(ReferenceErrorV1::NonFinite);
            }
            final_state[row_base + key] = updated;
            output[value] += updated * q_scaled[key];
        }
        if !output[value].is_finite() {
            return Err(ReferenceErrorV1::NonFinite);
        }
    }
    Ok(KimiK3KdaDecodeOutputV1 {
        final_state,
        output,
    })
}

fn kda_step_reference_v1(
    history: &[f32],
    gate_input: &[f32],
    state: &[f32],
    convolution_weights: &[f32],
) -> Result<KdaDecodeOutputV1, ReferenceErrorV1> {
    validate_finite_v1(history, KDA_TAPS_V1 * CHANNELS_V1)?;
    validate_finite_v1(gate_input, CHANNELS_V1)?;
    validate_finite_v1(state, CHANNELS_V1)?;
    validate_finite_v1(convolution_weights, KDA_TAPS_V1)?;

    let mut next = vec![0.0_f32; CHANNELS_V1];
    for channel in 0..CHANNELS_V1 {
        let convolution = (0..KDA_TAPS_V1)
            .map(|tap| history[tap * CHANNELS_V1 + channel] * convolution_weights[tap])
            .sum::<f32>();
        let proposal = (convolution + 0.25 * state[channel]).tanh();
        let gate = sigmoid_reference_v1(gate_input[channel])?;
        next[channel] = gate * state[channel] + (1.0 - gate) * proposal;
    }
    if next.iter().any(|value| !value.is_finite()) {
        return Err(ReferenceErrorV1::NonFinite);
    }
    let square_sum = next.iter().map(|value| value * value).sum::<f32>();
    let root = (square_sum / CHANNELS_V1 as f32 + RMS_EPSILON_V1).sqrt();
    if !root.is_finite() || root <= 0.0 {
        return Err(ReferenceErrorV1::DegenerateNormalization);
    }
    let normalized = next.iter().map(|value| value / root).collect();
    Ok(KdaDecodeOutputV1 {
        state: next,
        normalized,
    })
}

/// Evaluates one fixed three-tap KDA/GDN decode step.
pub fn kda_gdn_decode_reference_v1(
    history: &[f32],
    gate_input: &[f32],
    state: &[f32],
    convolution_weights: &[f32],
) -> Result<KdaDecodeOutputV1, ReferenceErrorV1> {
    kda_step_reference_v1(history, gate_input, state, convolution_weights)
}

/// Evaluates eight ordered KDA/GDN prefill steps in two four-token chunks.
pub fn kda_gdn_prefill_reference_v1(
    input: &[f32],
    gate_input: &[f32],
    initial_state: &[f32],
    convolution_weights: &[f32],
) -> Result<KdaPrefillOutputV1, ReferenceErrorV1> {
    validate_finite_v1(input, PREFILL_TOKENS_V1 * CHANNELS_V1)?;
    validate_finite_v1(gate_input, PREFILL_TOKENS_V1 * CHANNELS_V1)?;
    validate_finite_v1(initial_state, CHANNELS_V1)?;
    validate_finite_v1(convolution_weights, KDA_TAPS_V1)?;
    let mut state = initial_state.to_vec();
    let mut normalized = vec![0.0_f32; PREFILL_TOKENS_V1 * CHANNELS_V1];
    for chunk in 0..2 {
        for offset in 0..4 {
            let token = chunk * 4 + offset;
            let mut history = vec![0.0_f32; KDA_TAPS_V1 * CHANNELS_V1];
            for tap in 0..KDA_TAPS_V1 {
                if token >= tap {
                    history[tap * CHANNELS_V1..(tap + 1) * CHANNELS_V1].copy_from_slice(
                        &input[(token - tap) * CHANNELS_V1..(token - tap + 1) * CHANNELS_V1],
                    );
                }
            }
            let step = kda_step_reference_v1(
                &history,
                &gate_input[token * CHANNELS_V1..(token + 1) * CHANNELS_V1],
                &state,
                convolution_weights,
            )?;
            state = step.state;
            normalized[token * CHANNELS_V1..(token + 1) * CHANNELS_V1]
                .copy_from_slice(&step.normalized);
        }
    }
    Ok(KdaPrefillOutputV1 {
        final_state: state,
        normalized,
    })
}

/// Decodes OCP E4M3, including its positive and negative NaN encodings.
#[must_use]
pub fn decode_fp8_e4m3_reference_v1(value: u8) -> f32 {
    let exponent = ((value >> 3) & 15) as i32;
    let mantissa = (value & 7) as f32;
    if exponent == 15 && mantissa == 7.0 {
        return f32::NAN;
    }
    let magnitude = if exponent == 0 {
        mantissa * 2.0_f32.powi(-9)
    } else {
        (1.0 + mantissa / 8.0) * 2.0_f32.powi(exponent - 7)
    };
    if value & 0x80 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

fn attention_score_reference_v1(q: &[u8], k: &[u8], token: usize) -> Result<f32, ReferenceErrorV1> {
    let dot = (0..HEAD_DIMENSION_V1)
        .map(|depth| {
            decode_fp8_e4m3_reference_v1(q[depth])
                * decode_fp8_e4m3_reference_v1(k[token * HEAD_DIMENSION_V1 + depth])
        })
        .sum::<f32>();
    let score = dot * ATTENTION_SCALE_V1;
    score
        .is_finite()
        .then_some(score)
        .ok_or(ReferenceErrorV1::NonFinite)
}

fn sparse_selection_reference_v1(content_scores: &[f32]) -> [usize; SELECTED_TOKENS_V1] {
    let mut blocks = [0_usize, 1, 2, 3];
    blocks.sort_by(|left, right| {
        let left_score = content_scores
            [left * TOKENS_PER_BLOCK_V1..(left + 1) * TOKENS_PER_BLOCK_V1]
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let right_score = content_scores
            [right * TOKENS_PER_BLOCK_V1..(right + 1) * TOKENS_PER_BLOCK_V1]
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        if left_score > right_score {
            core::cmp::Ordering::Less
        } else if right_score > left_score {
            core::cmp::Ordering::Greater
        } else {
            left.cmp(right)
        }
    });
    let mut candidates = [0_usize; SELECTED_BLOCKS_V1 * TOKENS_PER_BLOCK_V1];
    for rank in 0..SELECTED_BLOCKS_V1 {
        for within in 0..TOKENS_PER_BLOCK_V1 {
            candidates[rank * TOKENS_PER_BLOCK_V1 + within] =
                blocks[rank] * TOKENS_PER_BLOCK_V1 + within;
        }
    }
    candidates.sort_by(|left, right| {
        if content_scores[*left] > content_scores[*right] {
            core::cmp::Ordering::Less
        } else if content_scores[*right] > content_scores[*left] {
            core::cmp::Ordering::Greater
        } else {
            left.cmp(right)
        }
    });
    [candidates[0], candidates[1], candidates[2]]
}

fn validate_attention_inputs_v1(q: &[u8], k: &[u8], v: &[u8]) -> Result<(), ReferenceErrorV1> {
    if q.len() != HEAD_DIMENSION_V1
        || k.len() != ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != ATTENTION_TOKENS_V1 * CHANNELS_V1
    {
        return Err(ReferenceErrorV1::Shape);
    }
    Ok(())
}

/// Evaluates fixed content-selected sparse attention.
pub fn content_sparse_attention_reference_v1(
    q: &[u8],
    k: &[u8],
    v: &[u8],
    content_scores: &[f32],
) -> Result<SparseAttentionOutputV1, ReferenceErrorV1> {
    validate_attention_inputs_v1(q, k, v)?;
    validate_finite_v1(content_scores, ATTENTION_TOKENS_V1)?;
    let selected = sparse_selection_reference_v1(content_scores);
    let mut scores = [0.0_f32; SELECTED_TOKENS_V1];
    for rank in 0..SELECTED_TOKENS_V1 {
        scores[rank] = attention_score_reference_v1(q, k, selected[rank])?
            + 0.75 * content_scores[selected[rank]];
    }
    let maximum = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let weights = scores.map(|score| (score - maximum).exp());
    let denominator = weights.iter().sum::<f32>();
    if !denominator.is_finite() || denominator <= 0.0 {
        return Err(ReferenceErrorV1::DegenerateNormalization);
    }
    let gate = sigmoid_reference_v1(maximum * 0.01)?;
    let mut output = vec![0.0_f32; CHANNELS_V1];
    for channel in 0..CHANNELS_V1 {
        for rank in 0..SELECTED_TOKENS_V1 {
            output[channel] += weights[rank] / denominator
                * decode_fp8_e4m3_reference_v1(v[selected[rank] * CHANNELS_V1 + channel]);
        }
        output[channel] *= gate;
    }
    Ok(SparseAttentionOutputV1 {
        selected: selected.map(|token| token as u32),
        output,
    })
}

/// Evaluates the fixed local plus compressed-global hybrid attention policy.
pub fn compressed_hybrid_attention_reference_v1(
    q: &[u8],
    k: &[u8],
    v: &[u8],
    token_bias: &[f32],
) -> Result<Vec<f32>, ReferenceErrorV1> {
    validate_attention_inputs_v1(q, k, v)?;
    validate_finite_v1(token_bias, ATTENTION_TOKENS_V1)?;
    let mut scores = [0.0_f32; ATTENTION_TOKENS_V1];
    for token in 0..ATTENTION_TOKENS_V1 {
        scores[token] = attention_score_reference_v1(q, k, token)? + token_bias[token];
    }
    let local_max = scores[12..16]
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let global_max = [scores[0], scores[4], scores[8]]
        .into_iter()
        .fold(f32::NEG_INFINITY, f32::max);
    let local_weights: [f32; TOKENS_PER_BLOCK_V1] =
        core::array::from_fn(|offset| (scores[12 + offset] - local_max).exp());
    let global_weights = [scores[0], scores[4], scores[8]].map(|score| (score - global_max).exp());
    let local_sum = local_weights.iter().sum::<f32>();
    let global_sum = global_weights.iter().sum::<f32>();
    if local_sum <= 0.0 || global_sum <= 0.0 || !local_sum.is_finite() || !global_sum.is_finite() {
        return Err(ReferenceErrorV1::DegenerateNormalization);
    }
    let mix = sigmoid_reference_v1(scores[0] * 0.01)?;
    let mut output = vec![0.0_f32; CHANNELS_V1];
    for channel in 0..CHANNELS_V1 {
        let local = (0..TOKENS_PER_BLOCK_V1)
            .map(|offset| {
                local_weights[offset] / local_sum
                    * decode_fp8_e4m3_reference_v1(v[(12 + offset) * CHANNELS_V1 + channel])
            })
            .sum::<f32>();
        let global = (0..3)
            .map(|block| {
                let compressed = (0..TOKENS_PER_BLOCK_V1)
                    .map(|within| {
                        decode_fp8_e4m3_reference_v1(
                            v[(block * TOKENS_PER_BLOCK_V1 + within) * CHANNELS_V1 + channel],
                        ) * 0.25
                    })
                    .sum::<f32>();
                global_weights[block] / global_sum * compressed
            })
            .sum::<f32>();
        output[channel] = mix * global + (1.0 - mix) * local;
    }
    Ok(output)
}

/// Evaluates four-depth per-channel AttnRes aggregation.
pub fn attnres_aggregate_reference_v1(
    depth_values: &[f32],
    depth_logits: &[f32],
) -> Result<Vec<f32>, ReferenceErrorV1> {
    validate_finite_v1(depth_values, MIXING_STREAMS_V1 * CHANNELS_V1)?;
    validate_finite_v1(depth_logits, MIXING_STREAMS_V1 * CHANNELS_V1)?;
    let mut output = vec![0.0_f32; CHANNELS_V1];
    for channel in 0..CHANNELS_V1 {
        let maximum = (0..MIXING_STREAMS_V1)
            .map(|depth| depth_logits[depth * CHANNELS_V1 + channel])
            .fold(f32::NEG_INFINITY, f32::max);
        let weights: [f32; MIXING_STREAMS_V1] = core::array::from_fn(|depth| {
            (depth_logits[depth * CHANNELS_V1 + channel] - maximum).exp()
        });
        let denominator = weights.iter().sum::<f32>();
        if denominator <= 0.0 || !denominator.is_finite() {
            return Err(ReferenceErrorV1::DegenerateNormalization);
        }
        output[channel] = (0..MIXING_STREAMS_V1)
            .map(|depth| weights[depth] * depth_values[depth * CHANNELS_V1 + channel])
            .sum::<f32>()
            / denominator;
    }
    Ok(output)
}

/// Evaluates four sigmoid-gated branches added to one residual.
pub fn four_branch_residual_reference_v1(
    residual: &[f32],
    branches: &[f32],
    gate_logits: &[f32],
) -> Result<Vec<f32>, ReferenceErrorV1> {
    validate_finite_v1(residual, CHANNELS_V1)?;
    validate_finite_v1(branches, MIXING_STREAMS_V1 * CHANNELS_V1)?;
    validate_finite_v1(gate_logits, MIXING_STREAMS_V1 * CHANNELS_V1)?;
    let mut output = residual.to_vec();
    for channel in 0..CHANNELS_V1 {
        for branch in 0..MIXING_STREAMS_V1 {
            output[channel] += 0.25
                * sigmoid_reference_v1(gate_logits[branch * CHANNELS_V1 + channel])?
                * branches[branch * CHANNELS_V1 + channel];
        }
    }
    if output.iter().any(|value| !value.is_finite()) {
        return Err(ReferenceErrorV1::NonFinite);
    }
    Ok(output)
}

/// Produces the normalized 4-by-4 mixing matrix used by mHC.
pub fn mhc_sinkhorn_matrix_reference_v1(
    mixing_logits: &[f32],
) -> Result<[f32; MIXING_STREAMS_V1 * MIXING_STREAMS_V1], ReferenceErrorV1> {
    validate_finite_v1(mixing_logits, MIXING_STREAMS_V1 * MIXING_STREAMS_V1)?;
    let mut matrix: [f32; MIXING_STREAMS_V1 * MIXING_STREAMS_V1] =
        core::array::from_fn(|index| mixing_logits[index].exp());
    for _ in 0..SINKHORN_ITERATIONS_V1 {
        for row in 0..MIXING_STREAMS_V1 {
            let sum = (0..MIXING_STREAMS_V1)
                .map(|column| matrix[row * MIXING_STREAMS_V1 + column])
                .sum::<f32>();
            if sum <= 0.0 || !sum.is_finite() {
                return Err(ReferenceErrorV1::DegenerateNormalization);
            }
            for column in 0..MIXING_STREAMS_V1 {
                matrix[row * MIXING_STREAMS_V1 + column] /= sum;
            }
        }
        for column in 0..MIXING_STREAMS_V1 {
            let sum = (0..MIXING_STREAMS_V1)
                .map(|row| matrix[row * MIXING_STREAMS_V1 + column])
                .sum::<f32>();
            if sum <= 0.0 || !sum.is_finite() {
                return Err(ReferenceErrorV1::DegenerateNormalization);
            }
            for row in 0..MIXING_STREAMS_V1 {
                matrix[row * MIXING_STREAMS_V1 + column] /= sum;
            }
        }
    }
    Ok(matrix)
}

/// Evaluates the fixed mHC Sinkhorn stream mix.
pub fn mhc_sinkhorn_mix_reference_v1(
    streams: &[f32],
    mixing_logits: &[f32],
) -> Result<Vec<f32>, ReferenceErrorV1> {
    validate_finite_v1(streams, MIXING_STREAMS_V1 * CHANNELS_V1)?;
    let matrix = mhc_sinkhorn_matrix_reference_v1(mixing_logits)?;
    let mut output = vec![0.0_f32; MIXING_STREAMS_V1 * CHANNELS_V1];
    for row in 0..MIXING_STREAMS_V1 {
        for channel in 0..CHANNELS_V1 {
            output[row * CHANNELS_V1 + channel] = (0..MIXING_STREAMS_V1)
                .map(|column| {
                    matrix[row * MIXING_STREAMS_V1 + column]
                        * streams[column * CHANNELS_V1 + channel]
                })
                .sum::<f32>();
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fp8_decoder_matches_fixture_values() {
        assert_eq!(decode_fp8_e4m3_reference_v1(0x00), 0.0);
        assert_eq!(decode_fp8_e4m3_reference_v1(0x38), 1.0);
        assert_eq!(decode_fp8_e4m3_reference_v1(0xb0), -0.5);
        assert!(decode_fp8_e4m3_reference_v1(0x7f).is_nan());
        assert!(decode_fp8_e4m3_reference_v1(0xff).is_nan());
    }

    #[test]
    fn non_finite_inputs_are_rejected() {
        let mut residual = vec![0.0; CHANNELS_V1];
        residual[3] = f32::NAN;
        assert_eq!(
            four_branch_residual_reference_v1(
                &residual,
                &vec![0.0; MIXING_STREAMS_V1 * CHANNELS_V1],
                &vec![0.0; MIXING_STREAMS_V1 * CHANNELS_V1],
            ),
            Err(ReferenceErrorV1::NonFinite)
        );
    }
}
