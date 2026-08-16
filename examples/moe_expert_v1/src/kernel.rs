//! Ordinary attributed Rust kernels for exact expert GEMM and combine.
//!
//! The definitions are real source, not explanatory text and not
//! `macro_rules!` expansions. Compiler and runtime authority remain absent.

#![allow(missing_docs)] // V1 generated typed-kernel modules lack rustdoc.

use fe2o3_device::{
    Bf16MfmaFragment, DeviceMatrix, DisjointSlice, F32AccumulatorFragment, Wave64, WaveLane,
    gfx942_lds_bf16_tile_pair_m16x16_v1, kernel, sync, thread,
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
    mut output: DisjointSlice<f32>,
) {
    let lane_index = thread::index_1d().get();
    if lane_index >= 64
        || activations.len() != MOE_EXPERT_TILE_ELEMENTS_V1
        || weights.len() != MOE_EXPERT_TILE_ELEMENTS_V1
        || output.len() != MOE_EXPERT_TILE_ELEMENTS_V1
    {
        fe2o3_device::trap();
        return;
    }

    let lane_column = lane_index % MOE_EXPERT_OUTPUT_WIDTH_V1;
    let depth_base = (lane_index / MOE_EXPERT_OUTPUT_WIDTH_V1) * 4;
    let activation_row_base = lane_column * MOE_EXPERT_INPUT_WIDTH_V1;
    let activation_fragment = Bf16MfmaFragment::from_bits([
        activations[activation_row_base + depth_base],
        activations[activation_row_base + depth_base + 1],
        activations[activation_row_base + depth_base + 2],
        activations[activation_row_base + depth_base + 3],
    ]);
    let weight_fragment = Bf16MfmaFragment::from_bits([
        weights[depth_base * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column],
        weights[(depth_base + 1) * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column],
        weights[(depth_base + 2) * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column],
        weights[(depth_base + 3) * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column],
    ]);

    // SAFETY: the fixed source profile admits exactly one gfx942 Wave64.
    let Some(lane) = (unsafe { WaveLane::<Wave64>::from_raw(lane_index as u32) }) else {
        fe2o3_device::trap();
        return;
    };
    // SAFETY: the exact profile provides two distinct aligned 512-byte tiles.
    let (mut activation_lds, mut weight_lds) = unsafe { gfx942_lds_bf16_tile_pair_m16x16_v1() };
    // SAFETY: each lane owns four distinct XOR4 locations in each tile.
    let activation_staged =
        unsafe { activation_lds.write_mfma_fragment(&lane, activation_fragment) };
    let weight_staged = unsafe { weight_lds.write_mfma_fragment(&lane, weight_fragment) };
    if !activation_staged || !weight_staged {
        fe2o3_device::trap();
        return;
    }
    // SAFETY: all 64 lanes execute the barrier after disjoint tile writes.
    unsafe { sync::syncthreads() };
    // SAFETY: the convergent barrier follows complete initialization.
    let activation_lds = unsafe { activation_lds.assume_init() };
    let weight_lds = unsafe { weight_lds.assume_init() };
    let Some(lhs) = activation_lds.read_mfma_fragment(lane_index) else {
        fe2o3_device::trap();
        return;
    };
    let Some(rhs) = weight_lds.read_mfma_fragment(lane_index) else {
        fe2o3_device::trap();
        return;
    };
    // SAFETY: exact gfx942 Wave64 source admission is required before lowering.
    let matrix = unsafe { DeviceMatrix::from_compiler() };
    let result =
        unsafe { matrix.multiply_accumulate(lhs, rhs, F32AccumulatorFragment::ZERO) }.into_values();

    // SAFETY: `(lane, component)` is a bijection over the 256 output elements.
    if let Some(slot) =
        unsafe { output.get_mut_at(depth_base * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column) }
    {
        *slot = result[0];
    }
    if let Some(slot) =
        unsafe { output.get_mut_at((depth_base + 1) * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column) }
    {
        *slot = result[1];
    }
    if let Some(slot) =
        unsafe { output.get_mut_at((depth_base + 2) * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column) }
    {
        *slot = result[2];
    }
    if let Some(slot) =
        unsafe { output.get_mut_at((depth_base + 3) * MOE_EXPERT_OUTPUT_WIDTH_V1 + lane_column) }
    {
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
