//! Independent safe CPU references for the fixed teaching profiles.

use crate::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, DEEPSEEK_SPARSE_TOP_K_V1, HEAD_DIMENSION_V1,
    KDA_CHUNK_TOKENS_V1, KDA_KEY_DIMENSION_V1, KDA_STATE_ELEMENTS_V1, KDA_VALUE_DIMENSION_V1,
    MIXING_STREAMS_V1, PREFILL_TOKENS_V1, SELECTED_BLOCKS_V1, SELECTED_TOKENS_V1,
    SINKHORN_ITERATIONS_V1, TOKENS_PER_BLOCK_V1,
};

const ATTENTION_SCALE_V1: f32 = 0.088_388_346;

/// Input-policy failures reported by the safe CPU references.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceErrorV1 {
    /// One or more slices do not match the exact fixed profile.
    Shape,
    /// An input or computed value is NaN or infinite.
    NonFinite,
    /// A normalization denominator is not finite and strictly positive.
    DegenerateNormalization,
    /// The selected-token list is empty after masking or repeats a valid token.
    InvalidSparseIndices,
    /// A preactivated KDA decay or step size is outside its documented domain.
    InvalidGate,
}

/// Matrix state and value output produced by one KDA decode step.
#[derive(Clone, Debug, PartialEq)]
pub struct KdaDecodeOutputV1 {
    /// Updated logical `[K,V]` matrix state.
    pub state: Vec<f32>,
    /// Value-vector result `S_next^T (q / sqrt(K))`.
    pub output: Vec<f32>,
}

/// State carry and all token outputs produced by two KDA chunks.
#[derive(Clone, Debug, PartialEq)]
pub struct KdaPrefillOutputV1 {
    /// Matrix state after the first four-token chunk.
    pub chunk_state: Vec<f32>,
    /// State after token seven.
    pub final_state: Vec<f32>,
    /// Token-major value outputs with shape `[8][16]`.
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

/// Output and stable-softmax state from the bounded DeepSeek DSA contract.
#[derive(Clone, Debug, PartialEq)]
pub struct DeepSeekSparseAttentionOutputV1 {
    /// One 16-channel attention output over only the valid selected tokens.
    pub output: Vec<f32>,
    /// Maximum selected-token logit used by stable softmax.
    pub softmax_maximum: f32,
    /// Sum of exponentials after subtracting `softmax_maximum`.
    pub softmax_normalizer: f32,
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

fn validate_kda_gates_v2(alpha: &[f32], beta: &[f32]) -> Result<(), ReferenceErrorV1> {
    if alpha.iter().any(|gate| *gate <= 0.0 || *gate > 1.0)
        || beta.iter().any(|step| *step < 0.0 || *step > 1.0)
    {
        return Err(ReferenceErrorV1::InvalidGate);
    }
    Ok(())
}

fn kda_matrix_step_f64_v2(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: f32,
    state: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let mut decayed = vec![0.0_f64; KDA_STATE_ELEMENTS_V1];
    for key_index in 0..KDA_KEY_DIMENSION_V1 {
        for value_index in 0..KDA_VALUE_DIMENSION_V1 {
            let index = key_index * KDA_VALUE_DIMENSION_V1 + value_index;
            decayed[index] = f64::from(alpha[key_index]) * state[index];
        }
    }
    let mut next = vec![0.0_f64; KDA_STATE_ELEMENTS_V1];
    let mut output = vec![0.0_f64; KDA_VALUE_DIMENSION_V1];
    for value_index in 0..KDA_VALUE_DIMENSION_V1 {
        let prediction = (0..KDA_KEY_DIMENSION_V1)
            .map(|key_index| {
                f64::from(key[key_index])
                    * decayed[key_index * KDA_VALUE_DIMENSION_V1 + value_index]
            })
            .sum::<f64>();
        let error = f64::from(value[value_index]) - prediction;
        for key_index in 0..KDA_KEY_DIMENSION_V1 {
            let index = key_index * KDA_VALUE_DIMENSION_V1 + value_index;
            next[index] = decayed[index] + f64::from(beta) * f64::from(key[key_index]) * error;
            output[value_index] += 0.25 * f64::from(query[key_index]) * next[index];
        }
    }
    (next, output)
}

/// Evaluates one matrix-state KDA step with independent f64 scalar loops.
pub fn kda_decode_reference_v2(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
) -> Result<KdaDecodeOutputV1, ReferenceErrorV1> {
    validate_finite_v1(query, KDA_KEY_DIMENSION_V1)?;
    validate_finite_v1(key, KDA_KEY_DIMENSION_V1)?;
    validate_finite_v1(value, KDA_VALUE_DIMENSION_V1)?;
    validate_finite_v1(alpha, KDA_KEY_DIMENSION_V1)?;
    validate_finite_v1(beta, 1)?;
    validate_finite_v1(initial_state, KDA_STATE_ELEMENTS_V1)?;
    validate_kda_gates_v2(alpha, beta)?;
    let initial = initial_state
        .iter()
        .map(|entry| f64::from(*entry))
        .collect::<Vec<_>>();
    let (state, output) = kda_matrix_step_f64_v2(query, key, value, alpha, beta[0], &initial);
    if state.iter().chain(&output).any(|entry| !entry.is_finite()) {
        return Err(ReferenceErrorV1::NonFinite);
    }
    Ok(KdaDecodeOutputV1 {
        state: state.into_iter().map(|entry| entry as f32).collect(),
        output: output.into_iter().map(|entry| entry as f32).collect(),
    })
}

/// Evaluates eight scalar KDA steps, independently of the GPU WY/UT transform.
pub fn kda_prefill_reference_v2(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
) -> Result<KdaPrefillOutputV1, ReferenceErrorV1> {
    validate_finite_v1(query, PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1)?;
    validate_finite_v1(key, PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1)?;
    validate_finite_v1(value, PREFILL_TOKENS_V1 * KDA_VALUE_DIMENSION_V1)?;
    validate_finite_v1(alpha, PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1)?;
    validate_finite_v1(beta, PREFILL_TOKENS_V1)?;
    validate_finite_v1(initial_state, KDA_STATE_ELEMENTS_V1)?;
    validate_kda_gates_v2(alpha, beta)?;
    let mut state = initial_state
        .iter()
        .map(|entry| f64::from(*entry))
        .collect::<Vec<_>>();
    let mut output = vec![0.0_f32; PREFILL_TOKENS_V1 * KDA_VALUE_DIMENSION_V1];
    let mut chunk_state = Vec::new();
    for token in 0..PREFILL_TOKENS_V1 {
        let key_start = token * KDA_KEY_DIMENSION_V1;
        let value_start = token * KDA_VALUE_DIMENSION_V1;
        let (next, token_output) = kda_matrix_step_f64_v2(
            &query[key_start..key_start + KDA_KEY_DIMENSION_V1],
            &key[key_start..key_start + KDA_KEY_DIMENSION_V1],
            &value[value_start..value_start + KDA_VALUE_DIMENSION_V1],
            &alpha[key_start..key_start + KDA_KEY_DIMENSION_V1],
            beta[token],
            &state,
        );
        state = next;
        output[value_start..value_start + KDA_VALUE_DIMENSION_V1].copy_from_slice(
            &token_output
                .into_iter()
                .map(|entry| entry as f32)
                .collect::<Vec<_>>(),
        );
        if token + 1 == KDA_CHUNK_TOKENS_V1 {
            chunk_state = state.iter().map(|entry| *entry as f32).collect();
        }
    }
    if state.iter().any(|entry| !entry.is_finite()) || output.iter().any(|entry| !entry.is_finite())
    {
        return Err(ReferenceErrorV1::NonFinite);
    }
    Ok(KdaPrefillOutputV1 {
        chunk_state,
        final_state: state.into_iter().map(|entry| entry as f32).collect(),
        output,
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

/// Evaluates the token-indexed attention stage of DeepSeek sparse attention.
///
/// The learned Lightning Indexer is the producer of `indices`; values greater
/// than or equal to the KV-cache token count are masked like FlashMLA's `-1`
/// sentinel. Valid entries must be unique and at least one must remain.
pub fn deepseek_sparse_attention_reference_v1(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    indices: &[u32],
) -> Result<DeepSeekSparseAttentionOutputV1, ReferenceErrorV1> {
    validate_finite_v1(q, HEAD_DIMENSION_V1)?;
    validate_finite_v1(k, ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1)?;
    validate_finite_v1(v, ATTENTION_TOKENS_V1 * CHANNELS_V1)?;
    if indices.len() != DEEPSEEK_SPARSE_TOP_K_V1 {
        return Err(ReferenceErrorV1::Shape);
    }

    let mut valid = [false; DEEPSEEK_SPARSE_TOP_K_V1];
    let mut tokens = [0_usize; DEEPSEEK_SPARSE_TOP_K_V1];
    let mut valid_count = 0;
    for rank in 0..DEEPSEEK_SPARSE_TOP_K_V1 {
        let token = indices[rank] as usize;
        if token >= ATTENTION_TOKENS_V1 {
            continue;
        }
        for previous in 0..rank {
            if valid[previous] && tokens[previous] == token {
                return Err(ReferenceErrorV1::InvalidSparseIndices);
            }
        }
        valid[rank] = true;
        tokens[rank] = token;
        valid_count += 1;
    }
    if valid_count == 0 {
        return Err(ReferenceErrorV1::InvalidSparseIndices);
    }

    let mut scores = [f32::NEG_INFINITY; DEEPSEEK_SPARSE_TOP_K_V1];
    for rank in 0..DEEPSEEK_SPARSE_TOP_K_V1 {
        if valid[rank] {
            let dot = (0..HEAD_DIMENSION_V1)
                .map(|depth| q[depth] * k[tokens[rank] * HEAD_DIMENSION_V1 + depth])
                .sum::<f32>();
            scores[rank] = dot * ATTENTION_SCALE_V1;
            if !scores[rank].is_finite() {
                return Err(ReferenceErrorV1::NonFinite);
            }
        }
    }
    let softmax_maximum = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut weights = [0.0_f32; DEEPSEEK_SPARSE_TOP_K_V1];
    for rank in 0..DEEPSEEK_SPARSE_TOP_K_V1 {
        if valid[rank] {
            weights[rank] = (scores[rank] - softmax_maximum).exp();
        }
    }
    let softmax_normalizer = weights.iter().sum::<f32>();
    if !softmax_normalizer.is_finite() || softmax_normalizer <= 0.0 {
        return Err(ReferenceErrorV1::DegenerateNormalization);
    }

    let mut output = vec![0.0_f32; CHANNELS_V1];
    for channel in 0..CHANNELS_V1 {
        for rank in 0..DEEPSEEK_SPARSE_TOP_K_V1 {
            if valid[rank] {
                output[channel] += weights[rank] * v[tokens[rank] * CHANNELS_V1 + channel];
            }
        }
        output[channel] /= softmax_normalizer;
        if !output[channel].is_finite() {
            return Err(ReferenceErrorV1::NonFinite);
        }
    }
    Ok(DeepSeekSparseAttentionOutputV1 {
        output,
        softmax_maximum,
        softmax_normalizer,
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
