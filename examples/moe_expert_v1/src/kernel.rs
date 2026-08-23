//! Ordinary attributed Rust kernels for exact expert GEMM and combine.
//!
//! The definitions are real source, not explanatory text and not
//! `macro_rules!` expansions. Compiler and runtime authority remain absent.

#![allow(missing_docs)] // V1 generated typed-kernel modules lack rustdoc.

use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, Blocked, DeviceMatrix, DisjointSlice,
    F32AccumulatorFragment, Index1D, Wave64, WaveLane, gfx942_lds_bf16_tile_pair_m16x16_v1,
    gfx942_publish_lds_bf16_tile_pair_m16x16_v1, kernel, thread,
};

use crate::contract::{
    DROP_ROUTE_V1, MOE_COMBINED_OUTPUT_ELEMENTS_V1, MOE_EXPERT_INPUT_WIDTH_V1,
    MOE_EXPERT_OUTPUT_WIDTH_V1, MOE_EXPERT_TILE_ELEMENTS_V1, MOE_ROUTES_PER_TOKEN_V1,
    MOE_ROUTES_V1,
};

/// Computes one host-selected expert's padded `16x16x16` BF16/F32 tile.
///
/// The host schedules this exact kernel four times. Only rows below the
/// expert's admitted count contain compacted token activations; all remaining
/// rows are required to contain BF16 +0 padding.
#[kernel(
    typed,
    namespace = "222c78b29207fdc638c693ee6a170e728e9acd790821ca9c3d2d0b9dce183b5c",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn moe_expert_gemm_bf16_m16_n16_k16_v1(
    activations: &[u16],
    weights: &[u16],
    mut output: DisjointSlice<f32, Blocked<Index1D, 16, 4>>,
) {
    let thread_index = thread::index_1d();
    let lane_index = thread_index.get();
    if lane_index >= 64
        || activations.len() != MOE_EXPERT_TILE_ELEMENTS_V1
        || weights.len() != MOE_EXPERT_TILE_ELEMENTS_V1
        || output.len() != MOE_EXPERT_TILE_ELEMENTS_V1
    {
        fe2o3_device::trap();
        return;
    }

    let lane = WaveLane::<Wave64>::current();
    let Ok(activation_matrix) = Bf16MfmaAMatrix::row_major(
        activations,
        0,
        MOE_EXPERT_OUTPUT_WIDTH_V1,
        MOE_EXPERT_INPUT_WIDTH_V1,
        MOE_EXPERT_INPUT_WIDTH_V1,
    ) else {
        fe2o3_device::trap();
        return;
    };
    let Ok(weight_matrix) = Bf16MfmaBMatrix::row_major(
        weights,
        0,
        MOE_EXPERT_INPUT_WIDTH_V1,
        MOE_EXPERT_OUTPUT_WIDTH_V1,
        MOE_EXPERT_OUTPUT_WIDTH_V1,
    ) else {
        fe2o3_device::trap();
        return;
    };
    let activation_fragment = activation_matrix.load_m16k16(&lane, 0, 0);
    let weight_fragment = weight_matrix.load_k16n16(&lane, 0, 0);
    let (mut activation_lds, mut weight_lds) = gfx942_lds_bf16_tile_pair_m16x16_v1();
    activation_lds.write_mfma_fragment(&lane, activation_fragment);
    weight_lds.write_mfma_fragment(&lane, weight_fragment);
    let (activation_lds, weight_lds) =
        gfx942_publish_lds_bf16_tile_pair_m16x16_v1(activation_lds, weight_lds);
    let lhs = activation_lds.read_mfma_fragment(&lane);
    let rhs = weight_lds.read_mfma_fragment(&lane);
    let matrix = DeviceMatrix::current();
    let result = matrix
        .multiply_accumulate(lhs, rhs, F32AccumulatorFragment::zero(&lane))
        .into_values();
    let Some(output_block) = thread_index.checked_block::<16, 4>() else {
        fe2o3_device::trap();
        return;
    };

    if let Some(slot) = output.get_block_mut(&output_block, 0) {
        *slot = result[0];
    }
    if let Some(slot) = output.get_block_mut(&output_block, 1) {
        *slot = result[1];
    }
    if let Some(slot) = output.get_block_mut(&output_block, 2) {
        *slot = result[2];
    }
    if let Some(slot) = output.get_block_mut(&output_block, 3) {
        *slot = result[3];
    }
}

/// Inverse-permutes accepted route rows and combines them in route-rank order.
///
/// `compact_output` is indexed by routing compact slot, `inverse` is indexed
/// by token-major/rank-minor route ID, and `route_weights` uses that same route
/// order. Dropped routes contribute zero and are not renormalized.
#[kernel(
    typed,
    namespace = "f26cfdf5303f4cf4f897e7408cdef56fbd1dd7947fcc64386480278a3deb0feb",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(2))
)]
pub fn moe_expert_combine_f32_t8_k2_o16_v1(
    compact_output: &[f32],
    inverse: &[u32],
    route_weights: &[f32],
    mut combined_output: DisjointSlice<f32>,
) {
    let index = thread::index_1d();
    let flat = index.get();
    if compact_output.len() != MOE_ROUTES_V1 * MOE_EXPERT_OUTPUT_WIDTH_V1
        || inverse.len() != MOE_ROUTES_V1
        || route_weights.len() != MOE_ROUTES_V1
        || combined_output.len() != MOE_COMBINED_OUTPUT_ELEMENTS_V1
    {
        fe2o3_device::trap();
        return;
    }
    if flat >= MOE_COMBINED_OUTPUT_ELEMENTS_V1 {
        return;
    }

    let token = flat / MOE_EXPERT_OUTPUT_WIDTH_V1;
    let output_column = flat % MOE_EXPERT_OUTPUT_WIDTH_V1;
    let first_route = token * MOE_ROUTES_PER_TOKEN_V1;
    let first_weight = route_weights[first_route];
    let second_weight = route_weights[first_route + 1];
    if !first_weight.is_finite()
        || !second_weight.is_finite()
        || first_weight < 0.0
        || second_weight < 0.0
        || first_weight + second_weight != 1.0
    {
        fe2o3_device::trap();
        return;
    }

    let mut accumulator = 0.0_f32;
    let mut rank = 0;
    while rank < MOE_ROUTES_PER_TOKEN_V1 {
        let route = first_route + rank;
        let slot = inverse[route];
        if slot != DROP_ROUTE_V1 {
            if slot as usize >= MOE_ROUTES_V1 {
                fe2o3_device::trap();
                return;
            }
            let compact_index = slot as usize * MOE_EXPERT_OUTPUT_WIDTH_V1 + output_column;
            accumulator += route_weights[route] * compact_output[compact_index];
        }
        rank += 1;
    }
    if let Some(output) = combined_output.get_mut(index) {
        *output = accumulator;
    }
}
