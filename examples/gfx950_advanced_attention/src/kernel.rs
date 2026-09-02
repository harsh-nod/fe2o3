//! Ordinary attributed Rust for the bounded advanced-attention profiles.

#![allow(missing_docs)] // The kernel macro emits an undocumented helper module.

#[cfg(target_arch = "amdgpu")]
use fe2o3_device::{
    kernel, Gfx950F32AccumulatorFragment, Gfx950Fp8E4M3, Gfx950Fp8MfmaAMatrix,
    Gfx950LdsTransposeTile, Gfx950Matrix, Gfx950Subgroup, Gfx950TransposeUninitialized, Index1D,
    KernelError, KernelResult, StridedReadView2D, Wave64, WaveLane,
};
use fe2o3_device::{thread, DeviceMath, DisjointSlice};
#[cfg(not(target_arch = "amdgpu"))]
use fe2o3_device::{GridExclusive, GridLeader};

#[cfg(target_arch = "amdgpu")]
use crate::KDA_KEY_DIMENSION_V1;
use crate::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, DEEPSEEK_SPARSE_TOP_K_V1, HEAD_DIMENSION_V1,
    KDA_STATE_ELEMENTS_V1, KDA_VALUE_DIMENSION_V1, MIXING_STREAMS_V1, PREFILL_TOKENS_V1,
    SELECTED_BLOCKS_V1, SELECTED_TOKENS_V1, SINKHORN_ITERATIONS_V1, SPARSE_BLOCKS_V1,
    TOKENS_PER_BLOCK_V1,
};

const ATTENTION_SCALE_V1: f32 = 0.088_388_346;

#[cfg(not(target_arch = "amdgpu"))]
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

#[cfg(not(target_arch = "amdgpu"))]
fn sigmoid_v1(math: &DeviceMath, value: f32) -> Option<f32> {
    let exponential = math.exp_f32(-value);
    let result = 1.0 / (1.0 + exponential);
    result.is_finite().then_some(result)
}

#[cfg(not(target_arch = "amdgpu"))]
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

#[cfg(any(target_arch = "amdgpu", test))]
macro_rules! decode_fp8_e4m3_v1 {
    ($value:expr) => {{
        let bits = $value;
        let exponent = (bits >> 3_u8) & 0xf;
        let mantissa = bits & 0x7;
        let magnitude = if exponent == 0xf && mantissa == 0x7 {
            let nan_source = f32::from(mantissa ^ 7_u8);
            nan_source / nan_source
        } else if exponent == 0 {
            f32::from(mantissa) / 512.0
        } else {
            let exponent0 = f32::from(exponent & 0x1);
            let exponent1 = f32::from((exponent >> 1_u8) & 0x1);
            let exponent2 = f32::from((exponent >> 2_u8) & 0x1);
            let exponent3 = f32::from((exponent >> 3_u8) & 0x1);
            let scale = (1.0 + exponent0)
                * (1.0 + 3.0 * exponent1)
                * (1.0 + 15.0 * exponent2)
                * (1.0 + 255.0 * exponent3)
                / 128.0;
            (1.0 + f32::from(mantissa) / 8.0) * scale
        };
        if bits & 0x80 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }};
}

#[cfg(target_arch = "amdgpu")]
macro_rules! consider_sparse_candidate_v1 {
    ($id:expr, $rank:expr, $attention:expr, $id0:ident, $rank0:ident, $attention0:ident,
     $id1:ident, $rank1:ident, $attention1:ident,
     $id2:ident, $rank2:ident, $attention2:ident) => {{
        let candidate_rank = $rank;
        let candidate_attention = $attention;
        if candidate_rank > $rank0 {
            $id2 = $id1;
            $rank2 = $rank1;
            $attention2 = $attention1;
            $id1 = $id0;
            $rank1 = $rank0;
            $attention1 = $attention0;
            $id0 = $id;
            $rank0 = candidate_rank;
            $attention0 = candidate_attention;
        } else if candidate_rank > $rank1 {
            $id2 = $id1;
            $rank2 = $rank1;
            $attention2 = $attention1;
            $id1 = $id;
            $rank1 = candidate_rank;
            $attention1 = candidate_attention;
        } else if candidate_rank > $rank2 {
            $id2 = $id;
            $rank2 = candidate_rank;
            $attention2 = candidate_attention;
        }
    }};
}

#[cfg(not(target_arch = "amdgpu"))]
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

#[cfg(not(target_arch = "amdgpu"))]
fn write_f32_v1(
    output: &mut DisjointSlice<f32, GridExclusive>,
    leader: &GridLeader,
    index: usize,
    value: f32,
) {
    let Some(slot) = output.get_mut_exclusive(leader, index) else {
        fe2o3_device::trap();
    };
    *slot = value;
}

#[cfg(not(target_arch = "amdgpu"))]
fn write_u32_v1(
    output: &mut DisjointSlice<u32, GridExclusive>,
    leader: &GridLeader,
    index: usize,
    value: u32,
) {
    let Some(slot) = output.get_mut_exclusive(leader, index) else {
        fe2o3_device::trap();
    };
    *slot = value;
}
#[cfg(target_arch = "amdgpu")]
macro_rules! kda_chunk_wy_v1 {
    ($base:expr, $query:ident, $key:ident, $value:ident, $alpha:ident, $beta:ident,
     $subgroup:ident, $key_index:ident, $value_column:ident, $state:ident,
     $output0:ident, $output1:ident, $output2:ident, $output3:ident) => {{
        let token0 = $base;
        let token1 = $base + 1;
        let token2 = $base + 2;
        let token3 = $base + 3;
        let q0 = 0.25 * $query.load_or(token0, $key_index, 0.0);
        let q1 = 0.25 * $query.load_or(token1, $key_index, 0.0);
        let q2 = 0.25 * $query.load_or(token2, $key_index, 0.0);
        let q3 = 0.25 * $query.load_or(token3, $key_index, 0.0);
        let k0 = $key.load_or(token0, $key_index, 0.0);
        let k1 = $key.load_or(token1, $key_index, 0.0);
        let k2 = $key.load_or(token2, $key_index, 0.0);
        let k3 = $key.load_or(token3, $key_index, 0.0);
        let a0 = $alpha.load_or(token0, $key_index, 0.0);
        let a1 = $alpha.load_or(token1, $key_index, 0.0);
        let a2 = $alpha.load_or(token2, $key_index, 0.0);
        let a3 = $alpha.load_or(token3, $key_index, 0.0);
        let c0 = a0;
        let c1 = a0 * a1;
        let c2 = c1 * a2;
        let c3 = c2 * a3;
        let h0 = $subgroup.reduce_sum_f32::<16>(c0 * k0 * $state);
        let h1 = $subgroup.reduce_sum_f32::<16>(c1 * k1 * $state);
        let h2 = $subgroup.reduce_sum_f32::<16>(c2 * k2 * $state);
        let h3 = $subgroup.reduce_sum_f32::<16>(c3 * k3 * $state);
        let beta0 = $beta.load_or(0, token0, 0.0);
        let beta1 = $beta.load_or(0, token1, 0.0);
        let beta2 = $beta.load_or(0, token2, 0.0);
        let beta3 = $beta.load_or(0, token3, 0.0);
        let l10 = beta1 * $subgroup.reduce_sum_f32::<16>(a1 * k1 * k0);
        let l20 = beta2 * $subgroup.reduce_sum_f32::<16>(a1 * a2 * k2 * k0);
        let l21 = beta2 * $subgroup.reduce_sum_f32::<16>(a2 * k2 * k1);
        let l30 = beta3 * $subgroup.reduce_sum_f32::<16>(a1 * a2 * a3 * k3 * k0);
        let l31 = beta3 * $subgroup.reduce_sum_f32::<16>(a2 * a3 * k3 * k1);
        let l32 = beta3 * $subgroup.reduce_sum_f32::<16>(a3 * k3 * k2);
        let z0 = beta0 * ($value.load_or(token0, $value_column, 0.0) - h0);
        let z1 = beta1 * ($value.load_or(token1, $value_column, 0.0) - h1) - l10 * z0;
        let z2 = beta2 * ($value.load_or(token2, $value_column, 0.0) - h2) - l20 * z0 - l21 * z1;
        let z3 = beta3 * ($value.load_or(token3, $value_column, 0.0) - h3)
            - l30 * z0
            - l31 * z1
            - l32 * z2;
        let base0 = $subgroup.reduce_sum_f32::<16>(c0 * q0 * $state);
        let base1 = $subgroup.reduce_sum_f32::<16>(c1 * q1 * $state);
        let base2 = $subgroup.reduce_sum_f32::<16>(c2 * q2 * $state);
        let base3 = $subgroup.reduce_sum_f32::<16>(c3 * q3 * $state);
        let r00 = $subgroup.reduce_sum_f32::<16>(q0 * k0);
        let r10 = $subgroup.reduce_sum_f32::<16>(a1 * q1 * k0);
        let r11 = $subgroup.reduce_sum_f32::<16>(q1 * k1);
        let r20 = $subgroup.reduce_sum_f32::<16>(a1 * a2 * q2 * k0);
        let r21 = $subgroup.reduce_sum_f32::<16>(a2 * q2 * k1);
        let r22 = $subgroup.reduce_sum_f32::<16>(q2 * k2);
        let r30 = $subgroup.reduce_sum_f32::<16>(a1 * a2 * a3 * q3 * k0);
        let r31 = $subgroup.reduce_sum_f32::<16>(a2 * a3 * q3 * k1);
        let r32 = $subgroup.reduce_sum_f32::<16>(a3 * q3 * k2);
        let r33 = $subgroup.reduce_sum_f32::<16>(q3 * k3);
        $output0 = base0 + r00 * z0;
        $output1 = base1 + r10 * z0 + r11 * z1;
        $output2 = base2 + r20 * z0 + r21 * z1 + r22 * z2;
        $output3 = base3 + r30 * z0 + r31 * z1 + r32 * z2 + r33 * z3;
        $state = c3 * $state + a1 * a2 * a3 * k0 * z0 + a2 * a3 * k1 * z1 + a3 * k2 * z2 + k3 * z3;
    }};
}

/// Evaluates one exact matrix-state Kimi Delta Attention decode step.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-kda-decode",
    not(feature = "kernel-kda-decode-baseline-v1")
))]
#[kernel(
    typed,
    namespace = "e249ff03f475aa75595229ee6a68e816a2a9ad395940c495ad874c54c0e9b0ad",
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_kda_decode(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
    mut final_state: DisjointSlice<f32, Index1D>,
    mut output: DisjointSlice<f32, Index1D>,
) {
    if query.len() != KDA_KEY_DIMENSION_V1
        || key.len() != KDA_KEY_DIMENSION_V1
        || value.len() != KDA_VALUE_DIMENSION_V1
        || alpha.len() != KDA_KEY_DIMENSION_V1
        || beta.len() != 1
        || initial_state.len() != KDA_STATE_ELEMENTS_V1
        || final_state.len() != KDA_STATE_ELEMENTS_V1
        || output.len() != KDA_STATE_ELEMENTS_V1
    {
        return;
    }
    let Ok(query) = StridedReadView2D::from_shared_slice(query, 0, 1, 16, 16) else {
        return;
    };
    let Ok(key) = StridedReadView2D::from_shared_slice(key, 0, 1, 16, 16) else {
        return;
    };
    let Ok(value) = StridedReadView2D::from_shared_slice(value, 0, 1, 16, 16) else {
        return;
    };
    let Ok(alpha) = StridedReadView2D::from_shared_slice(alpha, 0, 1, 16, 16) else {
        return;
    };
    let Ok(beta) = StridedReadView2D::from_shared_slice(beta, 0, 1, 1, 1) else {
        return;
    };
    let Ok(state) = StridedReadView2D::from_shared_slice(initial_state, 0, 16, 16, 16) else {
        return;
    };
    #[cfg(not(feature = "kernel-kda-decode-baseline-v1"))]
    {
        let linear = thread::index_1d().get();
        let key_index = linear & 15;
        let value_column = linear >> 4;
        let subgroup = Gfx950Subgroup::current();
        let query_value = query.load_or(0, key_index, 0.0);
        let key_value = key.load_or(0, key_index, 0.0);
        let alpha_value = alpha.load_or(0, key_index, 0.0);
        let value_input = value.load_or(0, value_column, 0.0);
        let step = beta.load_or(0, 0, 0.0);
        let decay = alpha_value * state.load_or(value_column, key_index, 0.0);
        let prediction = subgroup.reduce_sum_f32::<16>(key_value * decay);
        let error = value_input - prediction;
        let updated = decay + step * key_value * error;
        let result = subgroup.reduce_sum_f32::<16>(0.25 * query_value * updated);
        if let Some(slot) = final_state.get_mut(thread::index_1d()) {
            *slot = updated;
        }
        if let Some(slot) = output.get_mut(thread::index_1d()) {
            *slot = result;
        }
    }
}

#[cfg(not(target_arch = "amdgpu"))]
pub fn gfx950_kda_decode(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
    mut final_state: DisjointSlice<f32, GridExclusive>,
    mut output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    let Ok(result) =
        crate::reference::kda_decode_reference_v2(query, key, value, alpha, beta, initial_state)
    else {
        fe2o3_device::trap();
    };
    if final_state.len() != KDA_STATE_ELEMENTS_V1 || output.len() != KDA_VALUE_DIMENSION_V1 {
        fe2o3_device::trap();
    }
    let mut index = 0;
    while index < KDA_STATE_ELEMENTS_V1 {
        write_f32_v1(&mut final_state, &leader, index, result.state[index]);
        index += 1;
    }
    index = 0;
    while index < KDA_VALUE_DIMENSION_V1 {
        write_f32_v1(&mut output, &leader, index, result.output[index]);
        index += 1;
    }
}

/// Evaluates two exact four-token WY/UT KDA chunks with one register state carry.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-kda-prefill",
    not(feature = "kernel-kda-prefill-baseline-v1")
))]
#[kernel(
    typed,
    namespace = "673210266e41c1a545820dbc0baec859659b5c1cf4d5e3e8ac6b5e542b4028d3",
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_kda_chunkwise_prefill(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
    mut final_state: DisjointSlice<f32, Index1D>,
    mut output_chunk0: DisjointSlice<f32, Index1D>,
    mut output_chunk1: DisjointSlice<f32, Index1D>,
) {
    if query.len() != PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || key.len() != PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || value.len() != PREFILL_TOKENS_V1 * KDA_VALUE_DIMENSION_V1
        || alpha.len() != PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || beta.len() != PREFILL_TOKENS_V1
        || initial_state.len() != KDA_STATE_ELEMENTS_V1
        || final_state.len() != KDA_STATE_ELEMENTS_V1
        || output_chunk0.len() != KDA_STATE_ELEMENTS_V1
        || output_chunk1.len() != KDA_STATE_ELEMENTS_V1
    {
        return;
    }
    let Ok(query) = StridedReadView2D::from_shared_slice(query, 0, 8, 16, 16) else {
        return;
    };
    let Ok(key) = StridedReadView2D::from_shared_slice(key, 0, 8, 16, 16) else {
        return;
    };
    let Ok(value) = StridedReadView2D::from_shared_slice(value, 0, 8, 16, 16) else {
        return;
    };
    let Ok(alpha) = StridedReadView2D::from_shared_slice(alpha, 0, 8, 16, 16) else {
        return;
    };
    let Ok(beta) = StridedReadView2D::from_shared_slice(beta, 0, 1, 8, 8) else {
        return;
    };
    let Ok(initial_state) = StridedReadView2D::from_shared_slice(initial_state, 0, 16, 16, 16)
    else {
        return;
    };
    let linear = thread::index_1d().get();
    let key_index = linear & 15;
    let value_column = linear >> 4;
    let subgroup = Gfx950Subgroup::current();
    let mut state = initial_state.load_or(value_column, key_index, 0.0);
    let mut c00 = 0.0;
    let mut c01 = 0.0;
    let mut c02 = 0.0;
    let mut c03 = 0.0;
    kda_chunk_wy_v1!(
        0,
        query,
        key,
        value,
        alpha,
        beta,
        subgroup,
        key_index,
        value_column,
        state,
        c00,
        c01,
        c02,
        c03
    );
    let mut c10 = 0.0;
    let mut c11 = 0.0;
    let mut c12 = 0.0;
    let mut c13 = 0.0;
    kda_chunk_wy_v1!(
        4,
        query,
        key,
        value,
        alpha,
        beta,
        subgroup,
        key_index,
        value_column,
        state,
        c10,
        c11,
        c12,
        c13
    );
    let selected0 = if key_index < 4 {
        c00
    } else if key_index < 8 {
        c01
    } else if key_index < 12 {
        c02
    } else {
        c03
    };
    let selected1 = if key_index < 4 {
        c10
    } else if key_index < 8 {
        c11
    } else if key_index < 12 {
        c12
    } else {
        c13
    };
    if let Some(slot) = output_chunk0.get_mut(thread::index_1d()) {
        *slot = selected0;
    }
    if let Some(slot) = output_chunk1.get_mut(thread::index_1d()) {
        *slot = selected1;
    }
    if let Some(slot) = final_state.get_mut(thread::index_1d()) {
        *slot = state;
    }
}

#[cfg(not(target_arch = "amdgpu"))]
pub fn gfx950_kda_chunkwise_prefill(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
    mut final_state: DisjointSlice<f32, GridExclusive>,
    mut output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    let Ok(result) =
        crate::reference::kda_prefill_reference_v2(query, key, value, alpha, beta, initial_state)
    else {
        fe2o3_device::trap();
    };
    if final_state.len() != KDA_STATE_ELEMENTS_V1
        || output.len() != PREFILL_TOKENS_V1 * KDA_VALUE_DIMENSION_V1
    {
        fe2o3_device::trap();
    }
    let mut index = 0;
    while index < KDA_STATE_ELEMENTS_V1 {
        write_f32_v1(&mut final_state, &leader, index, result.final_state[index]);
        index += 1;
    }
    index = 0;
    while index < PREFILL_TOKENS_V1 * KDA_VALUE_DIMENSION_V1 {
        write_f32_v1(&mut output, &leader, index, result.output[index]);
        index += 1;
    }
}

#[cfg(not(target_arch = "amdgpu"))]
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

#[cfg(not(target_arch = "amdgpu"))]
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

#[cfg(not(target_arch = "amdgpu"))]
fn deepseek_attention_score_v1(q: &[f32], k: &[f32], token: usize) -> Option<f32> {
    let mut dot = 0.0_f32;
    let mut depth = 0;
    while depth < HEAD_DIMENSION_V1 {
        dot += q[depth] * k[token * HEAD_DIMENSION_V1 + depth];
        if !dot.is_finite() {
            return None;
        }
        depth += 1;
    }
    let score = dot * ATTENTION_SCALE_V1;
    score.is_finite().then_some(score)
}

/// Selects two content blocks, retains three tokens, and computes one 16-value output.
#[cfg(all(target_arch = "amdgpu", feature = "kernel-content-sparse-attention"))]
#[cfg_attr(
    not(feature = "kernel-content-sparse-attention-reciprocal-reuse-v1"),
    kernel(
        typed,
        namespace = "8e4b6794b9080758a96900d9f3bedc81f043b9c733ce0348fd3d56ab46e4ccf7",
        launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
    )
)]
#[cfg_attr(
    feature = "kernel-content-sparse-attention-reciprocal-reuse-v1",
    kernel(
        typed,
        namespace = "f218efb0354f7130940595fdb01023c6a5ec4dfd290be3858934c069d0db78b7",
        launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
    )
)]
pub fn gfx950_content_sparse_attention(
    q: &[u8],
    k: &[u8],
    v: &[u8],
    content_scores: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
    mut selected_output: DisjointSlice<u32, Index1D>,
) {
    if q.len() < ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || k.len() < ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() < ATTENTION_TOKENS_V1 * CHANNELS_V1
        || content_scores.len() < ATTENTION_TOKENS_V1
        || output.len() < CHANNELS_V1
        || selected_output.len() < SELECTED_TOKENS_V1
    {
        fe2o3_device::trap();
    }
    let index = thread::index_1d();
    let column = index.get() % ATTENTION_TOKENS_V1;
    let lane = WaveLane::<Wave64>::current();
    let Ok(query) = Gfx950Fp8MfmaAMatrix::row_major(
        q,
        0,
        ATTENTION_TOKENS_V1,
        HEAD_DIMENSION_V1,
        HEAD_DIMENSION_V1,
    ) else {
        fe2o3_device::trap();
    };
    let query = query.load_m16k128(&lane, 0, 0);
    let Ok(key) = Gfx950Fp8MfmaAMatrix::row_major(
        k,
        0,
        ATTENTION_TOKENS_V1,
        HEAD_DIMENSION_V1,
        HEAD_DIMENSION_V1,
    ) else {
        fe2o3_device::trap();
    };
    let key = Gfx950LdsTransposeTile::<Gfx950Fp8E4M3, Gfx950TransposeUninitialized>::current(&lane)
        .stage_k_transposed(&key, 0, 0)
        .publish()
        .read_mfma_fragment();
    let accumulator = Gfx950F32AccumulatorFragment::<Gfx950Fp8E4M3>::zero(&lane);
    let scores = Gfx950Matrix::current()
        .multiply_accumulate_fp8(query, key, accumulator)
        .into_values();
    let Ok(content) = StridedReadView2D::from_shared_slice(
        content_scores,
        0,
        1,
        ATTENTION_TOKENS_V1,
        ATTENTION_TOKENS_V1,
    ) else {
        fe2o3_device::trap();
    };

    let subgroup = Gfx950Subgroup::current();
    let math = DeviceMath::current();
    let lane_content = content.load_or(0, column, f32::NEG_INFINITY);
    let lane_attention = scores[0] * ATTENTION_SCALE_V1 + 0.75 * lane_content;
    let c0 = subgroup.broadcast_f32::<16>(lane_content, 0);
    let c1 = subgroup.broadcast_f32::<16>(lane_content, 1);
    let c2 = subgroup.broadcast_f32::<16>(lane_content, 2);
    let c3 = subgroup.broadcast_f32::<16>(lane_content, 3);
    let c4 = subgroup.broadcast_f32::<16>(lane_content, 4);
    let c5 = subgroup.broadcast_f32::<16>(lane_content, 5);
    let c6 = subgroup.broadcast_f32::<16>(lane_content, 6);
    let c7 = subgroup.broadcast_f32::<16>(lane_content, 7);
    let c8 = subgroup.broadcast_f32::<16>(lane_content, 8);
    let c9 = subgroup.broadcast_f32::<16>(lane_content, 9);
    let c10 = subgroup.broadcast_f32::<16>(lane_content, 10);
    let c11 = subgroup.broadcast_f32::<16>(lane_content, 11);
    let c12 = subgroup.broadcast_f32::<16>(lane_content, 12);
    let c13 = subgroup.broadcast_f32::<16>(lane_content, 13);
    let c14 = subgroup.broadcast_f32::<16>(lane_content, 14);
    let c15 = subgroup.broadcast_f32::<16>(lane_content, 15);
    let a0 = subgroup.broadcast_f32::<16>(lane_attention, 0);
    let a1 = subgroup.broadcast_f32::<16>(lane_attention, 1);
    let a2 = subgroup.broadcast_f32::<16>(lane_attention, 2);
    let a3 = subgroup.broadcast_f32::<16>(lane_attention, 3);
    let a4 = subgroup.broadcast_f32::<16>(lane_attention, 4);
    let a5 = subgroup.broadcast_f32::<16>(lane_attention, 5);
    let a6 = subgroup.broadcast_f32::<16>(lane_attention, 6);
    let a7 = subgroup.broadcast_f32::<16>(lane_attention, 7);
    let a8 = subgroup.broadcast_f32::<16>(lane_attention, 8);
    let a9 = subgroup.broadcast_f32::<16>(lane_attention, 9);
    let a10 = subgroup.broadcast_f32::<16>(lane_attention, 10);
    let a11 = subgroup.broadcast_f32::<16>(lane_attention, 11);
    let a12 = subgroup.broadcast_f32::<16>(lane_attention, 12);
    let a13 = subgroup.broadcast_f32::<16>(lane_attention, 13);
    let a14 = subgroup.broadcast_f32::<16>(lane_attention, 14);
    let a15 = subgroup.broadcast_f32::<16>(lane_attention, 15);

    let mut block0 = c0;
    if c1 > block0 {
        block0 = c1;
    }
    if c2 > block0 {
        block0 = c2;
    }
    if c3 > block0 {
        block0 = c3;
    }
    let mut block1 = c4;
    if c5 > block1 {
        block1 = c5;
    }
    if c6 > block1 {
        block1 = c6;
    }
    if c7 > block1 {
        block1 = c7;
    }
    let mut block2 = c8;
    if c9 > block2 {
        block2 = c9;
    }
    if c10 > block2 {
        block2 = c10;
    }
    if c11 > block2 {
        block2 = c11;
    }
    let mut block3 = c12;
    if c13 > block3 {
        block3 = c13;
    }
    if c14 > block3 {
        block3 = c14;
    }
    if c15 > block3 {
        block3 = c15;
    }
    let mut first_block = 0;
    let mut first_block_score = block0;
    if block1 > first_block_score {
        first_block = 1;
        first_block_score = block1;
    }
    if block2 > first_block_score {
        first_block = 2;
        first_block_score = block2;
    }
    if block3 > first_block_score {
        first_block = 3;
    }
    let mut second_block = if first_block == 0 { 1 } else { 0 };
    let mut second_block_score = if second_block == 0 { block0 } else { block1 };
    if first_block != 1 && block1 > second_block_score {
        second_block = 1;
        second_block_score = block1;
    }
    if first_block != 2 && block2 > second_block_score {
        second_block = 2;
        second_block_score = block2;
    }
    if first_block != 3 && block3 > second_block_score {
        second_block = 3;
    }

    let keep0 = first_block == 0 || second_block == 0;
    let keep1 = first_block == 1 || second_block == 1;
    let keep2 = first_block == 2 || second_block == 2;
    let keep3 = first_block == 3 || second_block == 3;
    let e0 = if keep0 { c0 } else { f32::NEG_INFINITY };
    let e1 = if keep0 { c1 } else { f32::NEG_INFINITY };
    let e2 = if keep0 { c2 } else { f32::NEG_INFINITY };
    let e3 = if keep0 { c3 } else { f32::NEG_INFINITY };
    let e4 = if keep1 { c4 } else { f32::NEG_INFINITY };
    let e5 = if keep1 { c5 } else { f32::NEG_INFINITY };
    let e6 = if keep1 { c6 } else { f32::NEG_INFINITY };
    let e7 = if keep1 { c7 } else { f32::NEG_INFINITY };
    let e8 = if keep2 { c8 } else { f32::NEG_INFINITY };
    let e9 = if keep2 { c9 } else { f32::NEG_INFINITY };
    let e10 = if keep2 { c10 } else { f32::NEG_INFINITY };
    let e11 = if keep2 { c11 } else { f32::NEG_INFINITY };
    let e12 = if keep3 { c12 } else { f32::NEG_INFINITY };
    let e13 = if keep3 { c13 } else { f32::NEG_INFINITY };
    let e14 = if keep3 { c14 } else { f32::NEG_INFINITY };
    let e15 = if keep3 { c15 } else { f32::NEG_INFINITY };

    let mut selected0 = usize::MAX;
    let mut selected0_rank = f32::NEG_INFINITY;
    let mut selected0_attention = f32::NEG_INFINITY;
    let mut selected1 = usize::MAX;
    let mut selected1_rank = f32::NEG_INFINITY;
    let mut selected1_attention = f32::NEG_INFINITY;
    let mut selected2 = usize::MAX;
    let mut selected2_rank = f32::NEG_INFINITY;
    let mut selected2_attention = f32::NEG_INFINITY;
    consider_sparse_candidate_v1!(
        0,
        e0,
        a0,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        1,
        e1,
        a1,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        2,
        e2,
        a2,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        3,
        e3,
        a3,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        4,
        e4,
        a4,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        5,
        e5,
        a5,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        6,
        e6,
        a6,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        7,
        e7,
        a7,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        8,
        e8,
        a8,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        9,
        e9,
        a9,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        10,
        e10,
        a10,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        11,
        e11,
        a11,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        12,
        e12,
        a12,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        13,
        e13,
        a13,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        14,
        e14,
        a14,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        15,
        e15,
        a15,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    let selected_index = thread::index_1d();
    let selected_rank = selected_index.get();
    if selected_rank < SELECTED_TOKENS_V1 {
        let selected = if selected_rank == 0 {
            selected0
        } else if selected_rank == 1 {
            selected1
        } else {
            selected2
        };
        if let Some(slot) = selected_output.get_mut(selected_index) {
            *slot = selected as u32;
        }
    }

    let Ok(value) =
        StridedReadView2D::from_shared_slice(v, 0, ATTENTION_TOKENS_V1, CHANNELS_V1, CHANNELS_V1)
    else {
        fe2o3_device::trap();
    };
    let mut maximum = selected0_attention;
    if selected1_attention > maximum {
        maximum = selected1_attention;
    }
    if selected2_attention > maximum {
        maximum = selected2_attention;
    }
    let weight0 = math.exp_f32(selected0_attention - maximum);
    let weight1 = math.exp_f32(selected1_attention - maximum);
    let weight2 = math.exp_f32(selected2_attention - maximum);
    let denominator = weight0 + weight1 + weight2;
    #[cfg(not(feature = "kernel-content-sparse-attention-reciprocal-reuse-v1"))]
    let result = weight0 / denominator * decode_fp8_e4m3_v1!(value.load_or(selected0, column, 0))
        + weight1 / denominator * decode_fp8_e4m3_v1!(value.load_or(selected1, column, 0))
        + weight2 / denominator * decode_fp8_e4m3_v1!(value.load_or(selected2, column, 0));
    #[cfg(feature = "kernel-content-sparse-attention-reciprocal-reuse-v1")]
    let result = {
        let reciprocal = 1.0 / denominator;
        (weight0 * decode_fp8_e4m3_v1!(value.load_or(selected0, column, 0))
            + weight1 * decode_fp8_e4m3_v1!(value.load_or(selected1, column, 0))
            + weight2 * decode_fp8_e4m3_v1!(value.load_or(selected2, column, 0)))
            * reciprocal
    };
    let output_gate = 1.0 / (1.0 + math.exp_f32(-maximum * 0.01));
    if index.get() < CHANNELS_V1 {
        if let Some(slot) = output.get_mut(index) {
            *slot = result * output_gate;
        }
    }
}

#[cfg(not(target_arch = "amdgpu"))]
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
    };
    if !denominator.is_finite() || denominator <= 0.0 {
        fe2o3_device::trap();
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

/// Consumes Lightning Indexer top-k token IDs and evaluates only those KV rows.
#[cfg(all(target_arch = "amdgpu", feature = "kernel-deepseek-sparse-attention"))]
#[kernel(
    typed,
    namespace = "62a1ee5804a9926ebb929061195f2229630ebdaf5a13a19d17ce7ddb4fcbbbe3",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_deepseek_sparse_attention(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    index0: u32,
    index1: u32,
    index2: u32,
    index3: u32,
    mut output: DisjointSlice<f32, Index1D>,
    mut softmax_maximum_output: DisjointSlice<f32, Index1D>,
    mut softmax_normalizer_output: DisjointSlice<f32, Index1D>,
) {
    if q.len() != HEAD_DIMENSION_V1
        || k.len() != ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != ATTENTION_TOKENS_V1 * CHANNELS_V1
        || output.len() != CHANNELS_V1
        || softmax_maximum_output.len() != 1
        || softmax_normalizer_output.len() != 1
    {
        fe2o3_device::trap();
    }

    let raw0 = index0;
    let raw1 = index1;
    let raw2 = index2;
    let raw3 = index3;
    let valid0 = raw0 < ATTENTION_TOKENS_V1 as u32;
    let valid1 = raw1 < ATTENTION_TOKENS_V1 as u32;
    let valid2 = raw2 < ATTENTION_TOKENS_V1 as u32;
    let valid3 = raw3 < ATTENTION_TOKENS_V1 as u32;
    if !(valid0 || valid1 || valid2 || valid3)
        || (valid0 && valid1 && raw0 == raw1)
        || (valid0 && valid2 && raw0 == raw2)
        || (valid0 && valid3 && raw0 == raw3)
        || (valid1 && valid2 && raw1 == raw2)
        || (valid1 && valid3 && raw1 == raw3)
        || (valid2 && valid3 && raw2 == raw3)
    {
        fe2o3_device::trap();
    }
    let token0 = if valid0 { raw0 as usize } else { 0 };
    let token1 = if valid1 { raw1 as usize } else { 0 };
    let token2 = if valid2 { raw2 as usize } else { 0 };
    let token3 = if valid3 { raw3 as usize } else { 0 };
    let Ok(query_view) =
        StridedReadView2D::from_shared_slice(q, 0, 1, HEAD_DIMENSION_V1, HEAD_DIMENSION_V1)
    else {
        fe2o3_device::trap();
    };
    let Ok(key_view) = StridedReadView2D::from_shared_slice(
        k,
        0,
        ATTENTION_TOKENS_V1,
        HEAD_DIMENSION_V1,
        HEAD_DIMENSION_V1,
    ) else {
        fe2o3_device::trap();
    };
    let Ok(value_view) =
        StridedReadView2D::from_shared_slice(v, 0, ATTENTION_TOKENS_V1, CHANNELS_V1, CHANNELS_V1)
    else {
        fe2o3_device::trap();
    };

    let linear_index = thread::index_1d().get();
    let column = linear_index % CHANNELS_V1;
    let depth0 = column;
    let depth1 = column + CHANNELS_V1;
    let depth2 = column + 2 * CHANNELS_V1;
    let depth3 = column + 3 * CHANNELS_V1;
    let depth4 = column + 4 * CHANNELS_V1;
    let depth5 = column + 5 * CHANNELS_V1;
    let depth6 = column + 6 * CHANNELS_V1;
    let depth7 = column + 7 * CHANNELS_V1;
    let query0 = query_view.load_or(0, depth0, 0.0);
    let query1 = query_view.load_or(0, depth1, 0.0);
    let query2 = query_view.load_or(0, depth2, 0.0);
    let query3 = query_view.load_or(0, depth3, 0.0);
    let query4 = query_view.load_or(0, depth4, 0.0);
    let query5 = query_view.load_or(0, depth5, 0.0);
    let query6 = query_view.load_or(0, depth6, 0.0);
    let query7 = query_view.load_or(0, depth7, 0.0);
    let mut partial0 = 0.0_f32;
    let mut partial1 = 0.0_f32;
    let mut partial2 = 0.0_f32;
    let mut partial3 = 0.0_f32;
    if valid0 {
        partial0 = query0 * key_view.load_or(token0, depth0, 0.0)
            + query1 * key_view.load_or(token0, depth1, 0.0)
            + query2 * key_view.load_or(token0, depth2, 0.0)
            + query3 * key_view.load_or(token0, depth3, 0.0)
            + query4 * key_view.load_or(token0, depth4, 0.0)
            + query5 * key_view.load_or(token0, depth5, 0.0)
            + query6 * key_view.load_or(token0, depth6, 0.0)
            + query7 * key_view.load_or(token0, depth7, 0.0);
    }
    if valid1 {
        partial1 = query0 * key_view.load_or(token1, depth0, 0.0)
            + query1 * key_view.load_or(token1, depth1, 0.0)
            + query2 * key_view.load_or(token1, depth2, 0.0)
            + query3 * key_view.load_or(token1, depth3, 0.0)
            + query4 * key_view.load_or(token1, depth4, 0.0)
            + query5 * key_view.load_or(token1, depth5, 0.0)
            + query6 * key_view.load_or(token1, depth6, 0.0)
            + query7 * key_view.load_or(token1, depth7, 0.0);
    }
    if valid2 {
        partial2 = query0 * key_view.load_or(token2, depth0, 0.0)
            + query1 * key_view.load_or(token2, depth1, 0.0)
            + query2 * key_view.load_or(token2, depth2, 0.0)
            + query3 * key_view.load_or(token2, depth3, 0.0)
            + query4 * key_view.load_or(token2, depth4, 0.0)
            + query5 * key_view.load_or(token2, depth5, 0.0)
            + query6 * key_view.load_or(token2, depth6, 0.0)
            + query7 * key_view.load_or(token2, depth7, 0.0);
    }
    if valid3 {
        partial3 = query0 * key_view.load_or(token3, depth0, 0.0)
            + query1 * key_view.load_or(token3, depth1, 0.0)
            + query2 * key_view.load_or(token3, depth2, 0.0)
            + query3 * key_view.load_or(token3, depth3, 0.0)
            + query4 * key_view.load_or(token3, depth4, 0.0)
            + query5 * key_view.load_or(token3, depth5, 0.0)
            + query6 * key_view.load_or(token3, depth6, 0.0)
            + query7 * key_view.load_or(token3, depth7, 0.0);
    }

    let subgroup = Gfx950Subgroup::current();
    let score0 = if valid0 {
        subgroup.reduce_sum_f32::<16>(partial0) * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let score1 = if valid1 {
        subgroup.reduce_sum_f32::<16>(partial1) * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let score2 = if valid2 {
        subgroup.reduce_sum_f32::<16>(partial2) * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let score3 = if valid3 {
        subgroup.reduce_sum_f32::<16>(partial3) * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let mut maximum = score0;
    if score1 > maximum {
        maximum = score1;
    }
    if score2 > maximum {
        maximum = score2;
    }
    if score3 > maximum {
        maximum = score3;
    }

    let math = DeviceMath::current();
    let weight0 = if valid0 {
        math.exp_f32(score0 - maximum)
    } else {
        0.0
    };
    let weight1 = if valid1 {
        math.exp_f32(score1 - maximum)
    } else {
        0.0
    };
    let weight2 = if valid2 {
        math.exp_f32(score2 - maximum)
    } else {
        0.0
    };
    let weight3 = if valid3 {
        math.exp_f32(score3 - maximum)
    } else {
        0.0
    };
    let normalizer = weight0 + weight1 + weight2 + weight3;
    let mut numerator = 0.0_f32;
    if valid0 {
        numerator += weight0 * value_view.load_or(token0, column, 0.0);
    }
    if valid1 {
        numerator += weight1 * value_view.load_or(token1, column, 0.0);
    }
    if valid2 {
        numerator += weight2 * value_view.load_or(token2, column, 0.0);
    }
    if valid3 {
        numerator += weight3 * value_view.load_or(token3, column, 0.0);
    }
    if linear_index < CHANNELS_V1 {
        if let Some(slot) = output.get_mut(thread::index_1d()) {
            *slot = numerator / normalizer;
        }
    }
    if linear_index == 0 {
        if let Some(slot) = softmax_maximum_output.get_mut(thread::index_1d()) {
            *slot = maximum;
        }
        if let Some(slot) = softmax_normalizer_output.get_mut(thread::index_1d()) {
            *slot = normalizer;
        }
    }
}

#[cfg(not(target_arch = "amdgpu"))]
pub fn gfx950_deepseek_sparse_attention(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    indices: &[u32],
    mut output: DisjointSlice<f32, GridExclusive>,
    mut softmax_maximum_output: DisjointSlice<f32, GridExclusive>,
    mut softmax_normalizer_output: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if q.len() != HEAD_DIMENSION_V1
        || k.len() != ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != ATTENTION_TOKENS_V1 * CHANNELS_V1
        || indices.len() != DEEPSEEK_SPARSE_TOP_K_V1
        || output.len() != CHANNELS_V1
        || softmax_maximum_output.len() != 1
        || softmax_normalizer_output.len() != 1
    {
        fe2o3_device::trap();
    }
    let mut valid = [false; DEEPSEEK_SPARSE_TOP_K_V1];
    let mut tokens = [0_usize; DEEPSEEK_SPARSE_TOP_K_V1];
    let mut valid_count = 0;
    let mut rank = 0;
    while rank < DEEPSEEK_SPARSE_TOP_K_V1 {
        let token = indices[rank] as usize;
        if token < ATTENTION_TOKENS_V1 {
            let mut previous = 0;
            while previous < rank {
                if valid[previous] && tokens[previous] == token {
                    fe2o3_device::trap();
                }
                previous += 1;
            }
            valid[rank] = true;
            tokens[rank] = token;
            valid_count += 1;
        }
        rank += 1;
    }
    if valid_count == 0 {
        fe2o3_device::trap();
    }

    let mut scores = [f32::NEG_INFINITY; DEEPSEEK_SPARSE_TOP_K_V1];
    let mut maximum = f32::NEG_INFINITY;
    rank = 0;
    while rank < DEEPSEEK_SPARSE_TOP_K_V1 {
        if valid[rank] {
            let Some(score) = deepseek_attention_score_v1(q, k, tokens[rank]) else {
                fe2o3_device::trap();
            };
            scores[rank] = score;
            if score > maximum {
                maximum = score;
            }
        }
        rank += 1;
    }
    let math = DeviceMath::current();
    let mut weights = [0.0_f32; DEEPSEEK_SPARSE_TOP_K_V1];
    let mut normalizer = 0.0_f32;
    rank = 0;
    while rank < DEEPSEEK_SPARSE_TOP_K_V1 {
        if valid[rank] {
            weights[rank] = math.exp_f32(scores[rank] - maximum);
            normalizer += weights[rank];
        }
        rank += 1;
    }
    if !normalizer.is_finite() || normalizer <= 0.0 {
        fe2o3_device::trap();
    }
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        let mut numerator = 0.0_f32;
        rank = 0;
        while rank < DEEPSEEK_SPARSE_TOP_K_V1 {
            if valid[rank] {
                numerator += weights[rank] * v[tokens[rank] * CHANNELS_V1 + channel];
            }
            rank += 1;
        }
        write_f32_v1(&mut output, &leader, channel, numerator / normalizer);
        channel += 1;
    }
    write_f32_v1(&mut softmax_maximum_output, &leader, 0, maximum);
    write_f32_v1(&mut softmax_normalizer_output, &leader, 0, normalizer);
}

/// Mixes a four-token local window with three four-token compressed global blocks.
#[cfg(all(target_arch = "amdgpu", feature = "kernel-compressed-hybrid-attention"))]
#[cfg_attr(
    not(feature = "kernel-compressed-hybrid-attention-division-baseline-v1"),
    kernel(
        typed,
        namespace = "c8cf1919826911b62fad830db644250616be68fd3aa252db280fb6cbf9157d3b",
        launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
    )
)]
#[cfg_attr(
    feature = "kernel-compressed-hybrid-attention-division-baseline-v1",
    kernel(
        typed,
        namespace = "df561e677c408c086c041faff22c05436c173edc2e4f9deda3eeaca93dc2a32b",
        launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
    )
)]
pub fn gfx950_compressed_hybrid_attention(
    q: &[u8],
    k: &[u8],
    v: &[u8],
    token_bias: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
) {
    if q.len() != ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || k.len() != ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != ATTENTION_TOKENS_V1 * CHANNELS_V1
        || token_bias.len() != ATTENTION_TOKENS_V1
        || output.len() != CHANNELS_V1
    {
        fe2o3_device::trap();
    }
    let index = thread::index_1d();
    let column = index.get() % ATTENTION_TOKENS_V1;
    let lane = WaveLane::<Wave64>::current();
    let Ok(query) = Gfx950Fp8MfmaAMatrix::row_major(
        q,
        0,
        ATTENTION_TOKENS_V1,
        HEAD_DIMENSION_V1,
        HEAD_DIMENSION_V1,
    ) else {
        fe2o3_device::trap();
    };
    let query = query.load_m16k128(&lane, 0, 0);
    let Ok(key) = Gfx950Fp8MfmaAMatrix::row_major(
        k,
        0,
        ATTENTION_TOKENS_V1,
        HEAD_DIMENSION_V1,
        HEAD_DIMENSION_V1,
    ) else {
        fe2o3_device::trap();
    };
    let key = Gfx950LdsTransposeTile::<Gfx950Fp8E4M3, Gfx950TransposeUninitialized>::current(&lane)
        .stage_k_transposed(&key, 0, 0)
        .publish()
        .read_mfma_fragment();
    let accumulator = Gfx950F32AccumulatorFragment::<Gfx950Fp8E4M3>::zero(&lane);
    let scores = Gfx950Matrix::current()
        .multiply_accumulate_fp8(query, key, accumulator)
        .into_values();
    let Ok(value) =
        StridedReadView2D::from_shared_slice(v, 0, ATTENTION_TOKENS_V1, CHANNELS_V1, CHANNELS_V1)
    else {
        fe2o3_device::trap();
    };
    let Ok(bias) = StridedReadView2D::from_shared_slice(token_bias, 0, 1, 16, 16) else {
        fe2o3_device::trap();
    };
    let subgroup = Gfx950Subgroup::current();
    let math = DeviceMath::current();
    let score = scores[0] * ATTENTION_SCALE_V1 + bias.load_or(0, column, 0.0);
    let score0 = subgroup.broadcast_f32::<16>(score, 0);
    let score4 = subgroup.broadcast_f32::<16>(score, 4);
    let score8 = subgroup.broadcast_f32::<16>(score, 8);
    let score12 = subgroup.broadcast_f32::<16>(score, 12);
    let score13 = subgroup.broadcast_f32::<16>(score, 13);
    let score14 = subgroup.broadcast_f32::<16>(score, 14);
    let score15 = subgroup.broadcast_f32::<16>(score, 15);

    let mut local_maximum = score12;
    if score13 > local_maximum {
        local_maximum = score13;
    }
    if score14 > local_maximum {
        local_maximum = score14;
    }
    if score15 > local_maximum {
        local_maximum = score15;
    }
    let local_weight0 = math.exp_f32(score12 - local_maximum);
    let local_weight1 = math.exp_f32(score13 - local_maximum);
    let local_weight2 = math.exp_f32(score14 - local_maximum);
    let local_weight3 = math.exp_f32(score15 - local_maximum);
    let local_sum = local_weight0 + local_weight1 + local_weight2 + local_weight3;
    #[cfg(feature = "kernel-compressed-hybrid-attention-division-baseline-v1")]
    let local_value = local_weight0 / local_sum * decode_fp8_e4m3_v1!(value.load_or(12, column, 0))
        + local_weight1 / local_sum * decode_fp8_e4m3_v1!(value.load_or(13, column, 0))
        + local_weight2 / local_sum * decode_fp8_e4m3_v1!(value.load_or(14, column, 0))
        + local_weight3 / local_sum * decode_fp8_e4m3_v1!(value.load_or(15, column, 0));
    #[cfg(not(feature = "kernel-compressed-hybrid-attention-division-baseline-v1"))]
    let local_value = {
        let reciprocal = 1.0 / local_sum;
        (local_weight0 * decode_fp8_e4m3_v1!(value.load_or(12, column, 0))
            + local_weight1 * decode_fp8_e4m3_v1!(value.load_or(13, column, 0))
            + local_weight2 * decode_fp8_e4m3_v1!(value.load_or(14, column, 0))
            + local_weight3 * decode_fp8_e4m3_v1!(value.load_or(15, column, 0)))
            * reciprocal
    };

    let mut global_maximum = score0;
    if score4 > global_maximum {
        global_maximum = score4;
    }
    if score8 > global_maximum {
        global_maximum = score8;
    }
    let global_weight0 = math.exp_f32(score0 - global_maximum);
    let global_weight1 = math.exp_f32(score4 - global_maximum);
    let global_weight2 = math.exp_f32(score8 - global_maximum);
    let global_sum = global_weight0 + global_weight1 + global_weight2;
    let compressed0 = (decode_fp8_e4m3_v1!(value.load_or(0, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(1, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(2, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(3, column, 0)))
        * 0.25;
    let compressed1 = (decode_fp8_e4m3_v1!(value.load_or(4, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(5, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(6, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(7, column, 0)))
        * 0.25;
    let compressed2 = (decode_fp8_e4m3_v1!(value.load_or(8, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(9, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(10, column, 0))
        + decode_fp8_e4m3_v1!(value.load_or(11, column, 0)))
        * 0.25;
    #[cfg(feature = "kernel-compressed-hybrid-attention-division-baseline-v1")]
    let global_value = global_weight0 / global_sum * compressed0
        + global_weight1 / global_sum * compressed1
        + global_weight2 / global_sum * compressed2;
    #[cfg(not(feature = "kernel-compressed-hybrid-attention-division-baseline-v1"))]
    let global_value = (global_weight0 * compressed0
        + global_weight1 * compressed1
        + global_weight2 * compressed2)
        * (1.0 / global_sum);
    let mix = 1.0 / (1.0 + math.exp_f32(-score0 * 0.01));
    if index.get() < CHANNELS_V1 {
        if let Some(slot) = output.get_mut(index) {
            *slot = mix * global_value + (1.0 - mix) * local_value;
        }
    }
}

#[cfg(not(target_arch = "amdgpu"))]
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
    }
    let mut scores = [0.0_f32; ATTENTION_TOKENS_V1];
    let mut token = 0;
    while token < ATTENTION_TOKENS_V1 {
        let Some(dot) = attention_score_v1(q, k, token) else {
            fe2o3_device::trap();
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
    };
    if local_sum <= 0.0 || global_sum <= 0.0 || !local_sum.is_finite() || !global_sum.is_finite() {
        fe2o3_device::trap();
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
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-attnres-aggregate",
    not(feature = "kernel-attnres-aggregate-explicit-reuse-v1")
))]
#[kernel(
    typed,
    namespace = "0f1b91664465bf059b47aa1fda8168a1cb4901cbfb81fd4dc770184520fca412",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
    control_flow(loop_bounds(4, 4))
)]
pub fn gfx950_attnres_aggregate(
    depth_values: &[f32],
    depth_logits: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
) -> KernelResult {
    if depth_values.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || depth_logits.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || output.len() != CHANNELS_V1
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let channel = index.get();
    if channel >= CHANNELS_V1 {
        return Ok(());
    }
    let Ok(values) = StridedReadView2D::from_shared_slice(depth_values, 0, 4, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let Ok(logits) = StridedReadView2D::from_shared_slice(depth_logits, 0, 4, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let math = DeviceMath::current();
    let mut maximum = logits.load_or(0, channel, f32::NEG_INFINITY);
    for depth in 1..4 {
        let logit = logits.load_or(depth, channel, f32::NEG_INFINITY);
        if logit > maximum {
            maximum = logit;
        }
    }
    let mut denominator = 0.0;
    let mut value = 0.0;
    for depth in 0..4 {
        let weight = math.exp_f32(logits.load_or(depth, channel, f32::NEG_INFINITY) - maximum);
        denominator += weight;
        value += weight * values.load_or(depth, channel, 0.0);
    }
    if let Some(slot) = output.get_mut(index) {
        *slot = value / denominator;
    }
    Ok(())
}

#[cfg(not(target_arch = "amdgpu"))]
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
        }
        write_f32_v1(&mut output, &leader, channel, value / denominator);
        channel += 1;
    }
}

/// Adds four sigmoid-gated branches to one 16-channel residual.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-four-branch-residual",
    not(feature = "kernel-four-branch-residual-explicit-v1")
))]
#[kernel(
    typed,
    namespace = "5a21124887ab5e89f2893f9a688ddc75efe2cf1c40dfda56be36acb530d69326",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
    control_flow(loop_bounds(4))
)]
pub fn gfx950_four_branch_residual(
    residual: &[f32],
    branches: &[f32],
    gate_logits: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
) {
    if residual.len() != CHANNELS_V1
        || branches.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || gate_logits.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || output.len() != CHANNELS_V1
    {
        return;
    }
    let index = thread::index_1d();
    let channel = index.get();
    if channel >= CHANNELS_V1 {
        return;
    }
    let math = DeviceMath::current();
    let mut value = residual[channel];
    for branch in 0_usize..4 {
        let offset = branch.wrapping_mul(CHANNELS_V1).wrapping_add(channel);
        let gate = 1.0 / (1.0 + math.exp_f32(-gate_logits[offset]));
        value += 0.25 * gate * branches[offset];
    }
    if let Some(slot) = output.get_mut(index) {
        *slot = value;
    }
}

#[cfg(not(target_arch = "amdgpu"))]
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
    }
    let math = DeviceMath::current();
    let mut channel = 0;
    while channel < CHANNELS_V1 {
        let mut value = residual[channel];
        let mut branch = 0;
        while branch < MIXING_STREAMS_V1 {
            let Some(gate) = sigmoid_v1(&math, gate_logits[branch * CHANNELS_V1 + channel]) else {
                fe2o3_device::trap();
            };
            value += 0.25 * gate * branches[branch * CHANNELS_V1 + channel];
            branch += 1;
        }
        if !value.is_finite() {
            fe2o3_device::trap();
        }
        write_f32_v1(&mut output, &leader, channel, value);
        channel += 1;
    }
}

/// Runs three Sinkhorn row/column normalizations and mixes four input streams.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-mhc-sinkhorn-mix",
    not(feature = "kernel-mhc-sinkhorn-mix-scalar-v1")
))]
#[kernel(
    typed,
    namespace = "e2bce999a5fa1929fa89c847d6dade5511566efd3cffca3003a77d00e870fdbf",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
    control_flow(loop_bounds(3))
)]
pub fn gfx950_mhc_sinkhorn_mix(
    streams: &[f32],
    mixing_logits: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
) -> KernelResult {
    if streams.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || mixing_logits.len() != MIXING_STREAMS_V1 * MIXING_STREAMS_V1
        || output.len() != MIXING_STREAMS_V1 * CHANNELS_V1
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let linear = index.get();
    let math = DeviceMath::current();
    let subgroup = Gfx950Subgroup::current();
    let Ok(logits) = StridedReadView2D::from_shared_slice(mixing_logits, 0, 1, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let Ok(streams) = StridedReadView2D::from_shared_slice(streams, 0, 4, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let row = linear / CHANNELS_V1;
    let local_lane = linear % CHANNELS_V1;
    let matrix_index = local_lane.wrapping_add(row.wrapping_mul(MIXING_STREAMS_V1))
        % (MIXING_STREAMS_V1 * MIXING_STREAMS_V1);
    let mut matrix = math.exp_f32(logits.load_or(0, matrix_index, 0.0));
    for _iteration in 0..3 {
        let row_reciprocal = 1.0 / subgroup.reduce_sum_f32::<4>(matrix);
        matrix *= row_reciprocal;

        let column = (local_lane as u32) & 3;
        let column_sum = subgroup.broadcast_f32::<16>(matrix, column)
            + subgroup.broadcast_f32::<16>(matrix, column.wrapping_add(4) & 15)
            + subgroup.broadcast_f32::<16>(matrix, column.wrapping_add(8) & 15)
            + subgroup.broadcast_f32::<16>(matrix, column.wrapping_add(12) & 15);
        matrix *= 1.0 / column_sum;
    }
    let weight0 = subgroup.broadcast_f32::<16>(matrix, 0);
    let weight1 = subgroup.broadcast_f32::<16>(matrix, 1);
    let weight2 = subgroup.broadcast_f32::<16>(matrix, 2);
    let weight3 = subgroup.broadcast_f32::<16>(matrix, 3);
    let value = weight0 * streams.load_or(0, local_lane, 0.0)
        + weight1 * streams.load_or(1, local_lane, 0.0)
        + weight2 * streams.load_or(2, local_lane, 0.0)
        + weight3 * streams.load_or(3, local_lane, 0.0);
    if let Some(slot) = output.get_mut(index) {
        *slot = value;
    }
    Ok(())
}

#[cfg(not(target_arch = "amdgpu"))]
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
    }
    let math = DeviceMath::current();
    let mut matrix = [0.0_f32; MIXING_STREAMS_V1 * MIXING_STREAMS_V1];
    let mut index = 0;
    while index < matrix.len() {
        matrix[index] = math.exp_f32(mixing_logits[index]);
        if !matrix[index].is_finite() {
            fe2o3_device::trap();
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

#[cfg(test)]
mod tests {
    use crate::reference::decode_fp8_e4m3_reference_v1;

    #[test]
    fn target_fp8_e4m3_formula_matches_all_encodings() {
        for bits in u8::MIN..=u8::MAX {
            let actual = decode_fp8_e4m3_v1!(bits);
            let expected = decode_fp8_e4m3_reference_v1(bits);
            if matches!(bits, 0x7f | 0xff) {
                assert!(actual.is_nan(), "0x{bits:02x} must decode as NaN");
                assert!(expected.is_nan(), "reference 0x{bits:02x} must be NaN");
            } else {
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "finite E4M3FN encoding 0x{bits:02x} changed"
                );
            }
        }

        assert_eq!(decode_fp8_e4m3_v1!(0x00).to_bits(), 0.0_f32.to_bits());
        assert_eq!(decode_fp8_e4m3_v1!(0x80).to_bits(), (-0.0_f32).to_bits());
    }
}
