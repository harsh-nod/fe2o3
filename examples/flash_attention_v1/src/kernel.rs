//! Ordinary attributed Rust source for one exact causal FlashAttention profile.
//!
//! This is the complete fused algorithm, not explanatory pseudocode and not a
//! `macro_rules!` facade. Phase A does not claim compiler or hardware support.

#![allow(missing_docs)] // The V1 kernel macro emits an undocumented helper module.

use fe2o3_device::{Blocked, DeviceMath, DisjointSlice, Index1D, kernel, thread};

use crate::contract::{
    ATTENTION_SCALE_BITS_V1, FLASH_ATTENTION_HEAD_DIMENSION_V1, FLASH_ATTENTION_INPUT_ELEMENTS_V1,
    FLASH_ATTENTION_OUTPUT_ELEMENTS_PER_LANE_V1, FLASH_ATTENTION_WAVE_LANES_V1,
};

/// Exact workgroup dimensions required by the attributed source.
pub const FLASH_ATTENTION_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Exact grid dimensions required by the fixed source profile.
pub const FLASH_ATTENTION_GRID_V1: [u32; 3] = [1, 1, 1];
/// Whether source-authenticated compiler lowering exists for this profile.
pub const FLASH_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1: bool = false;
/// Current boundary after the source/oracle/proof-facing Phase A slice.
pub const FLASH_ATTENTION_SOURCE_BLOCKER_V1: &str =
    "compiler authentication, Kernel IR, descriptor, LLVM/LLD artifact, and runtime are pending";

fn inputs_are_finite_v1(q: &[f32], k: &[f32], v: &[f32]) -> bool {
    let mut index = 0;
    while index < FLASH_ATTENTION_INPUT_ELEMENTS_V1 {
        if !q[index].is_finite() || !k[index].is_finite() || !v[index].is_finite() {
            return false;
        }
        index += 1;
    }
    true
}

fn score_v1(q: &[f32], k: &[f32], query_row: usize, key_row: usize) -> Option<f32> {
    let mut dot = 0.0_f32;
    let mut feature = 0;
    while feature < FLASH_ATTENTION_HEAD_DIMENSION_V1 {
        let q_index = query_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature;
        let k_index = key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature;
        let product = q[q_index] * k[k_index];
        if !product.is_finite() {
            return None;
        }
        dot += product;
        if !dot.is_finite() {
            return None;
        }
        feature += 1;
    }
    let scaled = dot * f32::from_bits(ATTENTION_SCALE_BITS_V1);
    scaled.is_finite().then_some(scaled)
}

fn output_pair_v1(
    math: &DeviceMath,
    q: &[f32],
    k: &[f32],
    v: &[f32],
    query_row: usize,
    output_column: usize,
) -> Option<[f32; 2]> {
    let mut running_max = 0.0_f32;
    let mut running_sum = 0.0_f32;
    let mut numerator = [0.0_f32; 2];
    let mut key_row = 0;

    while key_row <= query_row {
        let score = score_v1(q, k, query_row, key_row)?;
        let value_index = key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + output_column;
        let values = [v[value_index], v[value_index + 1]];

        if key_row == 0 {
            running_max = score;
            running_sum = 1.0;
            numerator = values;
        } else {
            let next_max = if score > running_max {
                score
            } else {
                running_max
            };
            let previous_weight = math.exp_f32(running_max - next_max);
            let current_weight = math.exp_f32(score - next_max);
            if !previous_weight.is_finite() || !current_weight.is_finite() {
                return None;
            }

            running_sum = running_sum * previous_weight + current_weight;
            numerator[0] = numerator[0] * previous_weight + values[0] * current_weight;
            numerator[1] = numerator[1] * previous_weight + values[1] * current_weight;
            running_max = next_max;
        }

        if !running_max.is_finite()
            || !running_sum.is_finite()
            || running_sum <= 0.0
            || !numerator[0].is_finite()
            || !numerator[1].is_finite()
        {
            return None;
        }
        key_row += 1;
    }

    let output = [numerator[0] / running_sum, numerator[1] / running_sum];
    if output[0].is_finite() && output[1].is_finite() {
        Some(output)
    } else {
        None
    }
}

/// Computes causal FP32 `O = softmax(Q K^T / sqrt(16)) V` for one fixed head.
///
/// Q, K, V, and O are separate contiguous row-major `[8][16]` allocations.
/// The launch is exactly one `gfx942:xnack-` Wave64 workgroup. Lane `l` owns O
/// indices `2*l` and `2*l+1`; therefore all 128 output elements are written
/// exactly once and there is no shape or output tail. Query row `r` reads only
/// key/value rows `0..=r`. Dot products, online max/sum rescaling, and value
/// accumulation are sequential strict FP32. Finite inputs are required. A lane
/// traps before either owned write if its arithmetic produces a non-finite
/// intermediate.
#[kernel(
    typed,
    namespace = "4dfe870bb76dd32b49144ee70ec4925eab8677b7cbd1a1bfe99fa2294f85fec8",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn flash_attention_causal_f32_b1_h1_n8_d16_v1(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    mut output: DisjointSlice<f32, Blocked<Index1D, 1, 2>>,
) {
    let lane_index = thread::index_1d();
    let lane = lane_index.get();
    if lane >= FLASH_ATTENTION_WAVE_LANES_V1
        || q.len() != FLASH_ATTENTION_INPUT_ELEMENTS_V1
        || k.len() != FLASH_ATTENTION_INPUT_ELEMENTS_V1
        || v.len() != FLASH_ATTENTION_INPUT_ELEMENTS_V1
        || output.len() != FLASH_ATTENTION_INPUT_ELEMENTS_V1
        || !inputs_are_finite_v1(q, k, v)
    {
        fe2o3_device::trap();
        return;
    }

    let first_output = lane * FLASH_ATTENTION_OUTPUT_ELEMENTS_PER_LANE_V1;
    let query_row = first_output / FLASH_ATTENTION_HEAD_DIMENSION_V1;
    let output_column = first_output % FLASH_ATTENTION_HEAD_DIMENSION_V1;

    let math = DeviceMath::current();
    let Some(values) = output_pair_v1(&math, q, k, v, query_row, output_column) else {
        fe2o3_device::trap();
        return;
    };
    let Some(output_block) = lane_index.checked_block::<1, 2>() else {
        fe2o3_device::trap();
        return;
    };

    if let Some(first) = output.get_block_mut(&output_block, 0) {
        *first = values[0];
    }
    if let Some(second) = output.get_block_mut(&output_block, 1) {
        *second = values[1];
    }
}
