//! Safe Rust source for the bounded GPT-OSS-120B gfx950 decode megakernel.
//!
//! Review the fused kernel by phases: launch-wide validation, wave/item
//! ownership, checked input views, uniform router and attention collectives,
//! gfx950 MFMA expert compute, then capability-checked stores. Keeping those
//! boundaries visible is especially important in a megakernel, where an early
//! return in only some lanes could otherwise invalidate a later collective.

#![allow(missing_docs)]

use fe2o3_device::{
    Blocked, DisjointWrite, Global, Index1D, KernelContext, KernelError, KernelResult, ReadOnly,
    kernel,
};

use crate::capability_views::{
    compute_attention, compute_expert_streamed, compute_parallel_router_logits, select_wave_top4,
    store_output_tile, store_packed_route,
};

use crate::{
    ATTENTION_OUTPUT_ELEMENTS, CONTEXT_TOKENS, EXPERT_K_TILE, EXPERT_N_TILE,
    EXPERT_OUTPUT_ELEMENTS, EXPERTS, HEAD_DIM, HIDDEN_SIZE, MATRIX_ROWS, MXFP4_BLOCKS, VALUE_TILE,
};

/// Fuses top-4 routing, sink attention, and selected MXFP4 expert projection.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-gpt-oss-decode"))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
#[allow(clippy::too_many_arguments, clippy::many_single_char_names)]
pub fn gfx950_gpt_oss_120b_decode_megakernel_v1(
    mut context: KernelContext<'_>,
    hidden_f32: Global<'_, f32, ReadOnly>,
    router_f32: Global<'_, f32, ReadOnly>,
    query_bf16: Global<'_, u16, ReadOnly>,
    key_transposed_bf16: Global<'_, u16, ReadOnly>,
    value_f32: Global<'_, f32, ReadOnly>,
    sinks_f32: Global<'_, f32, ReadOnly>,
    expert_activation_blocks_fp4: Global<'_, u8, ReadOnly>,
    expert_weight_blocks_fp4: Global<'_, u8, ReadOnly>,
    activation_scales: Global<'_, f32, ReadOnly>,
    expert_weight_scales: Global<'_, f32, ReadOnly>,
    mut attention_output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
    mut expert_output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
    mut packed_top4: Global<'_, u32, DisjointWrite<Index1D>>,
) -> KernelResult {
    // Validate the complete launch contract before any lane reaches a broadcast or MFMA.
    if hidden_f32.len() < crate::PROFILE_ITEMS * HIDDEN_SIZE
        || router_f32.len() < EXPERTS * HIDDEN_SIZE
        || query_bf16.len() < crate::PROFILE_ITEMS * MATRIX_ROWS * HEAD_DIM
        || key_transposed_bf16.len() < crate::PROFILE_ITEMS * HEAD_DIM * CONTEXT_TOKENS
        || value_f32.len() < crate::PROFILE_ITEMS * CONTEXT_TOKENS * VALUE_TILE
        || sinks_f32.len() < crate::PROFILE_ITEMS * MATRIX_ROWS
        || expert_activation_blocks_fp4.len()
            < crate::PROFILE_ITEMS * MXFP4_BLOCKS * MATRIX_ROWS * EXPERT_K_TILE
        || expert_weight_blocks_fp4.len() < EXPERTS * MXFP4_BLOCKS * EXPERT_K_TILE * EXPERT_N_TILE
        || activation_scales.len() < crate::PROFILE_ITEMS * MXFP4_BLOCKS
        || expert_weight_scales.len() < EXPERTS * MXFP4_BLOCKS * EXPERT_N_TILE
        || attention_output.len() < ATTENTION_OUTPUT_ELEMENTS
        || expert_output.len() < EXPERT_OUTPUT_ELEMENTS
        || packed_top4.len() < crate::PACKED_ROUTE_ELEMENTS
    {
        return Err(KernelError::InvalidArgument);
    }

    // One Wave64 owns one independent profile item; lane identity stays wave-local.
    let index = context.invocation().index_1d();
    let global_index = index.get();
    let lane_index = global_index % crate::WAVE_SIZE;
    let item_index = global_index / crate::WAVE_SIZE;

    let [local_logit0, local_logit1] =
        compute_parallel_router_logits(&hidden_f32, &router_f32, item_index, lane_index)?;

    // Every lane executes the same broadcasts and maintains the same deterministic top-4 list.
    let [id0, id1, id2, id3] = select_wave_top4(&mut context, local_logit0, local_logit1);
    let selected = (id0 as usize) & (EXPERTS - 1);

    // Four BF16 MFMA operations cover K=64 while the accumulator remains in registers.
    let [attention0, attention1, attention2, attention3] = compute_attention(
        &mut context,
        &query_bf16,
        &key_transposed_bf16,
        &value_f32,
        &sinks_f32,
        item_index,
        lane_index,
    )?;

    // The selected expert uses four K=128 MXFP4 blocks with per-block product scales.
    let [expert_acc0, expert_acc1, expert_acc2, expert_acc3] = compute_expert_streamed(
        &mut context,
        &expert_activation_blocks_fp4,
        &expert_weight_blocks_fp4,
        &activation_scales,
        &expert_weight_scales,
        item_index,
        lane_index,
        selected,
    )?;

    // Wave16-shaped blocked capabilities preserve the row-major 16x16 tile layout.
    store_output_tile(
        &context,
        &mut attention_output,
        [attention0, attention1, attention2, attention3],
    )?;
    store_output_tile(
        &context,
        &mut expert_output,
        [expert_acc0, expert_acc1, expert_acc2, expert_acc3],
    )?;
    // Every lane has the same top-4 list; the 1-D capability keeps item stores disjoint.
    let packed = id0 | (id1 << 7) | (id2 << 14) | (id3 << 21);
    store_packed_route(&context, &mut packed_top4, packed)
}
