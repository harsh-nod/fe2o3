//! Functional control for the historical two-stage BF16 attention experiment.
//!
//! The current capability API cannot bind reusable, phase-branded Global-to-LDS
//! storage to the BF16 matrix fragment consumed by the next phase. This variant
//! therefore runs the canonical safe attention schedule and makes no pipeline
//! or performance claim. The historical feature and module names are retained
//! so existing qualification tooling can continue to select the control.

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

/// Runs the safe functional control for the unavailable double-buffered schedule.
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-gpt-oss-decode-pipelined-attention"
))]
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
    // Validate the entire item contract before any collective or pipeline operation.
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

    // One Wave64 owns one independent item.
    let index = context.invocation().index_1d();
    let global_index = index.get();
    let lane_index = global_index % crate::WAVE_SIZE;
    let item_index = global_index / crate::WAVE_SIZE;

    let [local_logit0, local_logit1] =
        compute_parallel_router_logits(&hidden_f32, &router_f32, item_index, lane_index)?;

    // Router broadcasts remain uniform and identical to the production kernel.
    let [id0, id1, id2, id3] = select_wave_top4(&mut context, local_logit0, local_logit1);
    let selected = (id0 as usize) & (EXPERTS - 1);

    // Functional control: the reusable branded LDS-to-MFMA bridge is unavailable.
    let [attention0, attention1, attention2, attention3] = compute_attention(
        &mut context,
        &query_bf16,
        &key_transposed_bf16,
        &value_f32,
        &sinks_f32,
        item_index,
        lane_index,
    )?;

    // Expert projection and stores match the canonical kernel.
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
    let packed = id0 | (id1 << 7) | (id2 << 14) | (id3 << 21);
    store_packed_route(&context, &mut packed_top4, packed)
}
