//! Ordinary attributed Rust for the bounded advanced-attention profiles.

#![allow(missing_docs)] // The kernel macro emits an undocumented helper module.

use fe2o3_device::{DeviceMath, DisjointSlice, GridExclusive, GridLeader, kernel, thread};

use crate::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, HEAD_DIMENSION_V1, KDA_TAPS_V1, MIXING_STREAMS_V1,
    PREFILL_TOKENS_V1, SELECTED_BLOCKS_V1, SELECTED_TOKENS_V1, SINKHORN_ITERATIONS_V1,
    SPARSE_BLOCKS_V1, TOKENS_PER_BLOCK_V1,
};

const ATTENTION_SCALE_V1: f32 = 0.088_388_346;
const RMS_EPSILON_V1: f32 = 1.0e-5;

fn finite_slice_v1(values: &[f32], expected: usize) -> bool {
    if values.len() != expected {
        return false;
    }
    let mut index = 0;
    while index < expected {
        if !values[index].is_finite() {
            return false;
        }
        index += 1;
    }
    true
}

fn sigmoid_v1(math: &DeviceMath, value: f32) -> Option<f32> {
    let exponential = math.exp_f32(-value);
    let result = 1.0 / (1.0 + exponential);
    result.is_finite().then_some(result)
}

fn tanh_v1(math: &DeviceMath, value: f32) -> Option<f32> {
    let sigmoid = 1.0 / (1.0 + math.exp_f32(-2.0 * value));
    let result = 2.0 * sigmoid - 1.0;
    result.is_finite().then_some(result)
}

fn decode_fp8_e4m3_v1(value: u8) -> f32 {
    let exponent = ((value >> 3) & 15) as i32;
    let mantissa = (value & 7) as f32;
    if exponent == 15 && mantissa == 7.0 {
        return f32::NAN;
    }
    let magnitude = if exponent == 0 {
        mantissa * (1.0 / 512.0)
    } else {
        (1.0 + mantissa * 0.125) * exp2_integer_v1(exponent - 7)
    };
    if value & 0x80 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

fn exp2_integer_v1(exponent: i32) -> f32 {
    let mut result = 1.0_f32;
    let mut step = 0;
    if exponent >= 0 {
        while step < exponent {
            result *= 2.0;
            step += 1;
        }
    } else {
        while step < -exponent {
            result *= 0.5;
            step += 1;
        }
    }
    result
}

fn write_f32_v1(
    output: &mut DisjointSlice<f32, GridExclusive>,
    leader: &GridLeader,
    index: usize,
    value: f32,
) {
    let Some(slot) = output.get_mut_exclusive(leader, index) else {
        fe2o3_device::trap();
        return;
    };
    *slot = value;
}

fn write_u32_v1(
    output: &mut DisjointSlice<u32, GridExclusive>,
    leader: &GridLeader,
    index: usize,
    value: u32,
) {
    let Some(slot) = output.get_mut_exclusive(leader, index) else {
        fe2o3_device::trap();
        return;
    };
    *slot = value;
}

fn kda_update_v1(
    math: &DeviceMath,
    history: &[f32],
    gate_input: &[f32],
    state: &[f32; CHANNELS_V1],
    weights: &[f32],
) -> Option<([f32; CHANNELS_V1], [f32; CHANNELS_V1])> {
    let mut next = [0.0_f32; CHANNELS_V1];
    let mut square_sum = 0.0_f32;
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        let mut convolution = 0.0_f32;
        let mut tap = 0;
        while tap < KDA_TAPS_V1 {
            convolution += history[tap * CHANNELS_V1 + channel] * weights[tap];
            tap += 1;
        }
        let proposal = tanh_v1(math, convolution + 0.25 * state[channel])?;
        let gate = sigmoid_v1(math, gate_input[channel])?;
        next[channel] = gate * state[channel] + (1.0 - gate) * proposal;
        square_sum += next[channel] * next[channel];
        if !next[channel].is_finite() || !square_sum.is_finite() {
            return None;
        }
        channel += 1;
    }
    let root = math.sqrt_f32(square_sum / CHANNELS_V1 as f32 + RMS_EPSILON_V1);
    if !root.is_finite() || root <= 0.0 {
        return None;
    }
    let mut normalized = [0.0_f32; CHANNELS_V1];
    channel = 0;
    while channel < CHANNELS_V1 {
        normalized[channel] = next[channel] / root;
        channel += 1;
    }
    Some((next, normalized))
}

/// Applies one three-tap gated recurrence and RMS-normalizes its 16-channel state.
#[kernel(
    typed,
    namespace = "ee41c04158f6f9bd299168ab92fbef9a9965b5496cbe795428506befe2d17e0d",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16))
)]
pub fn gfx950_kda_gdn_decode(
    history: &[f32],
    gate_input: &[f32],
    state: &[f32],
    convolution_weights: &[f32],
    mut state_output: DisjointSlice<f32, GridExclusive>,
    mut normalized_output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if !finite_slice_v1(history, KDA_TAPS_V1 * CHANNELS_V1)
        || !finite_slice_v1(gate_input, CHANNELS_V1)
        || !finite_slice_v1(state, CHANNELS_V1)
        || !finite_slice_v1(convolution_weights, KDA_TAPS_V1)
        || state_output.len() != CHANNELS_V1
        || normalized_output.len() != CHANNELS_V1
    {
        fe2o3_device::trap();
        return;
    }
    let mut initial = [0.0_f32; CHANNELS_V1];
    initial.copy_from_slice(state);
    let math = DeviceMath::current();
    let Some((next, normalized)) =
        kda_update_v1(&math, history, gate_input, &initial, convolution_weights)
    else {
        fe2o3_device::trap();
        return;
    };
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        write_f32_v1(&mut state_output, &leader, channel, next[channel]);
        write_f32_v1(
            &mut normalized_output,
            &leader,
            channel,
            normalized[channel],
        );
        channel += 1;
    }
}

/// Applies the same recurrence to eight ordered tokens in two four-token chunks.
#[kernel(
    typed,
    namespace = "d7eb54ccc0cde5d616c700746ffaabca1fc9e22f85e8b603a8505703c65b0898",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(2, 4, 3, 16, 16, 16))
)]
pub fn gfx950_kda_gdn_prefill(
    input: &[f32],
    gate_input: &[f32],
    initial_state: &[f32],
    convolution_weights: &[f32],
    mut final_state: DisjointSlice<f32, GridExclusive>,
    mut normalized_output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if !finite_slice_v1(input, PREFILL_TOKENS_V1 * CHANNELS_V1)
        || !finite_slice_v1(gate_input, PREFILL_TOKENS_V1 * CHANNELS_V1)
        || !finite_slice_v1(initial_state, CHANNELS_V1)
        || !finite_slice_v1(convolution_weights, KDA_TAPS_V1)
        || final_state.len() != CHANNELS_V1
        || normalized_output.len() != PREFILL_TOKENS_V1 * CHANNELS_V1
    {
        fe2o3_device::trap();
        return;
    }

    let math = DeviceMath::current();
    let mut state = [0.0_f32; CHANNELS_V1];
    state.copy_from_slice(initial_state);
    let mut history = [0.0_f32; KDA_TAPS_V1 * CHANNELS_V1];
    let mut chunk = 0;
    while chunk < 2 {
        let mut offset = 0;
        while offset < 4 {
            let token = chunk * 4 + offset;
            let mut tap = 0;
            while tap < KDA_TAPS_V1 {
                let mut channel = 0;
                while channel < CHANNELS_V1 {
                    history[tap * CHANNELS_V1 + channel] = if token >= tap {
                        input[(token - tap) * CHANNELS_V1 + channel]
                    } else {
                        0.0
                    };
                    channel += 1;
                }
                tap += 1;
            }
            let gates = &gate_input[token * CHANNELS_V1..(token + 1) * CHANNELS_V1];
            let Some((next, normalized)) =
                kda_update_v1(&math, &history, gates, &state, convolution_weights)
            else {
                fe2o3_device::trap();
                return;
            };
            state = next;
            let mut channel = 0;
            while channel < CHANNELS_V1 {
                write_f32_v1(
                    &mut normalized_output,
                    &leader,
                    token * CHANNELS_V1 + channel,
                    normalized[channel],
                );
                channel += 1;
            }
            offset += 1;
        }
        chunk += 1;
    }
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        write_f32_v1(&mut final_state, &leader, channel, state[channel]);
        channel += 1;
    }
}

fn select_sparse_tokens_v1(content_scores: &[f32]) -> [usize; SELECTED_TOKENS_V1] {
    let mut block_maxima = [f32::NEG_INFINITY; SPARSE_BLOCKS_V1];
    let mut block = 0;
    while block < SPARSE_BLOCKS_V1 {
        let mut within = 0;
        while within < TOKENS_PER_BLOCK_V1 {
            let score = content_scores[block * TOKENS_PER_BLOCK_V1 + within];
            if score > block_maxima[block] {
                block_maxima[block] = score;
            }
            within += 1;
        }
        block += 1;
    }
    let mut selected_blocks = [usize::MAX; SELECTED_BLOCKS_V1];
    let mut rank = 0;
    while rank < SELECTED_BLOCKS_V1 {
        let mut best = usize::MAX;
        block = 0;
        while block < SPARSE_BLOCKS_V1 {
            let duplicate = rank > 0 && selected_blocks[0] == block;
            if !duplicate && (best == usize::MAX || block_maxima[block] > block_maxima[best]) {
                best = block;
            }
            block += 1;
        }
        selected_blocks[rank] = best;
        rank += 1;
    }
    let mut selected_tokens = [usize::MAX; SELECTED_TOKENS_V1];
    rank = 0;
    while rank < SELECTED_TOKENS_V1 {
        let mut best = usize::MAX;
        let mut candidate = 0;
        while candidate < SELECTED_BLOCKS_V1 * TOKENS_PER_BLOCK_V1 {
            let candidate_block = selected_blocks[candidate / TOKENS_PER_BLOCK_V1];
            let token = candidate_block * TOKENS_PER_BLOCK_V1 + candidate % TOKENS_PER_BLOCK_V1;
            let mut duplicate = false;
            let mut previous = 0;
            while previous < rank {
                if selected_tokens[previous] == token {
                    duplicate = true;
                }
                previous += 1;
            }
            if !duplicate && (best == usize::MAX || content_scores[token] > content_scores[best]) {
                best = token;
            }
            candidate += 1;
        }
        selected_tokens[rank] = best;
        rank += 1;
    }
    selected_tokens
}

fn attention_score_v1(q: &[u8], k: &[u8], token: usize) -> Option<f32> {
    let mut dot = 0.0_f32;
    let mut depth = 0;
    while depth < HEAD_DIMENSION_V1 {
        dot +=
            decode_fp8_e4m3_v1(q[depth]) * decode_fp8_e4m3_v1(k[token * HEAD_DIMENSION_V1 + depth]);
        if !dot.is_finite() {
            return None;
        }
        depth += 1;
    }
    let score = dot * ATTENTION_SCALE_V1;
    score.is_finite().then_some(score)
}

/// Selects two content blocks, retains three tokens, and computes one 16-value output.
#[kernel(
    typed,
    namespace = "4113e541a94ca80028142dfcf53760b4ed3c36dc4fe567e8399b206f31620f10",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(3, 3, 3, 16, 3))
)]
pub fn gfx950_content_sparse_attention(
    q: &[u8],
    k: &[u8],
    v: &[u8],
    content_scores: &[f32],
    mut output: DisjointSlice<f32, GridExclusive>,
    mut selected_output: DisjointSlice<u32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if q.len() != HEAD_DIMENSION_V1
        || k.len() != ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != ATTENTION_TOKENS_V1 * CHANNELS_V1
        || !finite_slice_v1(content_scores, ATTENTION_TOKENS_V1)
        || output.len() != CHANNELS_V1
        || selected_output.len() != SELECTED_TOKENS_V1
    {
        fe2o3_device::trap();
        return;
    }
    let selected = select_sparse_tokens_v1(content_scores);
    let mut rank = 0;
    while rank < SELECTED_TOKENS_V1 {
        write_u32_v1(&mut selected_output, &leader, rank, selected[rank] as u32);
        rank += 1;
    }

    let mut scores = [0.0_f32; SELECTED_TOKENS_V1];
    let mut maximum = f32::NEG_INFINITY;
    rank = 0;
    while rank < SELECTED_TOKENS_V1 {
        let token = selected[rank];
        let Some(dot) = attention_score_v1(q, k, token) else {
            fe2o3_device::trap();
            return;
        };
        scores[rank] = dot + 0.75 * content_scores[token];
        if scores[rank] > maximum {
            maximum = scores[rank];
        }
        rank += 1;
    }

    let math = DeviceMath::current();
    let mut probabilities = [0.0_f32; SELECTED_TOKENS_V1];
    let mut denominator = 0.0_f32;
    rank = 0;
    while rank < SELECTED_TOKENS_V1 {
        probabilities[rank] = math.exp_f32(scores[rank] - maximum);
        denominator += probabilities[rank];
        rank += 1;
    }
    let Some(output_gate) = sigmoid_v1(&math, maximum * 0.01) else {
        fe2o3_device::trap();
        return;
    };
    if !denominator.is_finite() || denominator <= 0.0 {
        fe2o3_device::trap();
        return;
    }
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        let mut value = 0.0_f32;
        rank = 0;
        while rank < SELECTED_TOKENS_V1 {
            let token = selected[rank];
            value += probabilities[rank] / denominator
                * decode_fp8_e4m3_v1(v[token * CHANNELS_V1 + channel]);
            rank += 1;
        }
        write_f32_v1(&mut output, &leader, channel, value * output_gate);
        channel += 1;
    }
}

/// Mixes a four-token local window with three four-token compressed global blocks.
#[kernel(
    typed,
    namespace = "44be0e88b5376e713b306cdc4d347668cf7d3b235df5e1d2283c86825cc6cd0b",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16, 16, 3, 4, 3, 16, 4, 3, 4))
)]
pub fn gfx950_compressed_hybrid_attention(
    q: &[u8],
    k: &[u8],
    v: &[u8],
    token_bias: &[f32],
    mut output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if q.len() != HEAD_DIMENSION_V1
        || k.len() != ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != ATTENTION_TOKENS_V1 * CHANNELS_V1
        || !finite_slice_v1(token_bias, ATTENTION_TOKENS_V1)
        || output.len() != CHANNELS_V1
    {
        fe2o3_device::trap();
        return;
    }

    let mut scores = [0.0_f32; ATTENTION_TOKENS_V1];
    let mut token = 0;
    while token < ATTENTION_TOKENS_V1 {
        let Some(dot) = attention_score_v1(q, k, token) else {
            fe2o3_device::trap();
            return;
        };
        scores[token] = dot + token_bias[token];
        token += 1;
    }
    let mut local_max = f32::NEG_INFINITY;
    token = 12;
    while token < ATTENTION_TOKENS_V1 {
        if scores[token] > local_max {
            local_max = scores[token];
        }
        token += 1;
    }
    let mut global_max = f32::NEG_INFINITY;
    let mut block = 0;
    while block < 3 {
        if scores[block * TOKENS_PER_BLOCK_V1] > global_max {
            global_max = scores[block * TOKENS_PER_BLOCK_V1];
        }
        block += 1;
    }

    let math = DeviceMath::current();
    let mut local_weights = [0.0_f32; TOKENS_PER_BLOCK_V1];
    let mut global_weights = [0.0_f32; 3];
    let mut local_sum = 0.0_f32;
    let mut global_sum = 0.0_f32;
    let mut offset = 0;
    while offset < TOKENS_PER_BLOCK_V1 {
        local_weights[offset] = math.exp_f32(scores[12 + offset] - local_max);
        local_sum += local_weights[offset];
        offset += 1;
    }
    block = 0;
    while block < 3 {
        global_weights[block] = math.exp_f32(scores[block * TOKENS_PER_BLOCK_V1] - global_max);
        global_sum += global_weights[block];
        block += 1;
    }
    let Some(mix) = sigmoid_v1(&math, scores[0] * 0.01) else {
        fe2o3_device::trap();
        return;
    };
    if local_sum <= 0.0 || global_sum <= 0.0 || !local_sum.is_finite() || !global_sum.is_finite() {
        fe2o3_device::trap();
        return;
    }

    let mut channel = 0;
    while channel < CHANNELS_V1 {
        let mut local_value = 0.0_f32;
        offset = 0;
        while offset < TOKENS_PER_BLOCK_V1 {
            local_value += local_weights[offset] / local_sum
                * decode_fp8_e4m3_v1(v[(12 + offset) * CHANNELS_V1 + channel]);
            offset += 1;
        }
        let mut global_value = 0.0_f32;
        block = 0;
        while block < 3 {
            let mut compressed = 0.0_f32;
            offset = 0;
            while offset < TOKENS_PER_BLOCK_V1 {
                compressed += decode_fp8_e4m3_v1(
                    v[(block * TOKENS_PER_BLOCK_V1 + offset) * CHANNELS_V1 + channel],
                ) * 0.25;
                offset += 1;
            }
            global_value += global_weights[block] / global_sum * compressed;
            block += 1;
        }
        write_f32_v1(
            &mut output,
            &leader,
            channel,
            mix * global_value + (1.0 - mix) * local_value,
        );
        channel += 1;
    }
}

/// Softmax-aggregates four residual depths independently for each channel.
#[kernel(
    typed,
    namespace = "7e0cbfed6cef5800c858545c60a9af1077cc58adb44e7a30d45b24af2b06912c",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16, 4, 4))
)]
pub fn gfx950_attnres_aggregate(
    depth_values: &[f32],
    depth_logits: &[f32],
    mut output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if !finite_slice_v1(depth_values, MIXING_STREAMS_V1 * CHANNELS_V1)
        || !finite_slice_v1(depth_logits, MIXING_STREAMS_V1 * CHANNELS_V1)
        || output.len() != CHANNELS_V1
    {
        fe2o3_device::trap();
        return;
    }
    let math = DeviceMath::current();
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        let mut maximum = f32::NEG_INFINITY;
        let mut depth = 0;
        while depth < MIXING_STREAMS_V1 {
            let logit = depth_logits[depth * CHANNELS_V1 + channel];
            if logit > maximum {
                maximum = logit;
            }
            depth += 1;
        }
        let mut denominator = 0.0_f32;
        let mut value = 0.0_f32;
        depth = 0;
        while depth < MIXING_STREAMS_V1 {
            let weight = math.exp_f32(depth_logits[depth * CHANNELS_V1 + channel] - maximum);
            denominator += weight;
            value += weight * depth_values[depth * CHANNELS_V1 + channel];
            depth += 1;
        }
        if denominator <= 0.0 || !denominator.is_finite() || !value.is_finite() {
            fe2o3_device::trap();
            return;
        }
        write_f32_v1(&mut output, &leader, channel, value / denominator);
        channel += 1;
    }
}

/// Adds four sigmoid-gated branches to one 16-channel residual.
#[kernel(
    typed,
    namespace = "daa0d2be9c26f84a19bccf606b449abd952628599d5d1e6db996edc368dffa38",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16, 4))
)]
pub fn gfx950_four_branch_residual(
    residual: &[f32],
    branches: &[f32],
    gate_logits: &[f32],
    mut output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if !finite_slice_v1(residual, CHANNELS_V1)
        || !finite_slice_v1(branches, MIXING_STREAMS_V1 * CHANNELS_V1)
        || !finite_slice_v1(gate_logits, MIXING_STREAMS_V1 * CHANNELS_V1)
        || output.len() != CHANNELS_V1
    {
        fe2o3_device::trap();
        return;
    }
    let math = DeviceMath::current();
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        let mut value = residual[channel];
        let mut branch = 0;
        while branch < MIXING_STREAMS_V1 {
            let Some(gate) = sigmoid_v1(&math, gate_logits[branch * CHANNELS_V1 + channel]) else {
                fe2o3_device::trap();
                return;
            };
            value += 0.25 * gate * branches[branch * CHANNELS_V1 + channel];
            branch += 1;
        }
        if !value.is_finite() {
            fe2o3_device::trap();
            return;
        }
        write_f32_v1(&mut output, &leader, channel, value);
        channel += 1;
    }
}

/// Runs three Sinkhorn row/column normalizations and mixes four input streams.
#[kernel(
    typed,
    namespace = "3549ff86c422a24a0d67bac54b9fd644700d3a164d590f091f185bf1f0b47bae",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16, 3, 4, 4, 4, 4, 4, 4, 4, 16, 4))
)]
pub fn gfx950_mhc_sinkhorn_mix(
    streams: &[f32],
    mixing_logits: &[f32],
    mut output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if !finite_slice_v1(streams, MIXING_STREAMS_V1 * CHANNELS_V1)
        || !finite_slice_v1(mixing_logits, MIXING_STREAMS_V1 * MIXING_STREAMS_V1)
        || output.len() != MIXING_STREAMS_V1 * CHANNELS_V1
    {
        fe2o3_device::trap();
        return;
    }
    let math = DeviceMath::current();
    let mut matrix = [0.0_f32; MIXING_STREAMS_V1 * MIXING_STREAMS_V1];
    let mut index = 0;
    while index < matrix.len() {
        matrix[index] = math.exp_f32(mixing_logits[index]);
        if !matrix[index].is_finite() {
            fe2o3_device::trap();
            return;
        }
        index += 1;
    }
    let mut iteration = 0;
    while iteration < SINKHORN_ITERATIONS_V1 {
        let mut row = 0;
        while row < MIXING_STREAMS_V1 {
            let mut sum = 0.0_f32;
            let mut column = 0;
            while column < MIXING_STREAMS_V1 {
                sum += matrix[row * MIXING_STREAMS_V1 + column];
                column += 1;
            }
            if sum <= 0.0 || !sum.is_finite() {
                fe2o3_device::trap();
                return;
            }
            column = 0;
            while column < MIXING_STREAMS_V1 {
                matrix[row * MIXING_STREAMS_V1 + column] /= sum;
                column += 1;
            }
            row += 1;
        }
        let mut column = 0;
        while column < MIXING_STREAMS_V1 {
            let mut sum = 0.0_f32;
            let mut row = 0;
            while row < MIXING_STREAMS_V1 {
                sum += matrix[row * MIXING_STREAMS_V1 + column];
                row += 1;
            }
            if sum <= 0.0 || !sum.is_finite() {
                fe2o3_device::trap();
                return;
            }
            row = 0;
            while row < MIXING_STREAMS_V1 {
                matrix[row * MIXING_STREAMS_V1 + column] /= sum;
                row += 1;
            }
            column += 1;
        }
        iteration += 1;
    }
    let mut row = 0;
    while row < MIXING_STREAMS_V1 {
        let mut channel = 0;
        while channel < CHANNELS_V1 {
            let mut value = 0.0_f32;
            let mut column = 0;
            while column < MIXING_STREAMS_V1 {
                value += matrix[row * MIXING_STREAMS_V1 + column]
                    * streams[column * CHANNELS_V1 + channel];
                column += 1;
            }
            write_f32_v1(&mut output, &leader, row * CHANNELS_V1 + channel, value);
            channel += 1;
        }
        row += 1;
    }
}
