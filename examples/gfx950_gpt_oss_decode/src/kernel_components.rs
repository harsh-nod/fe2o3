//! Exact-semantics materialized GPT-OSS-120B layer-tile component kernels.
//!
//! These kernels split the megakernel at materialization boundaries without
//! changing its math. Review each as: validate, establish wave/item ownership,
//! construct checked views, execute uniform collectives, then store through a
//! disjoint capability. The shared phase structure makes fused and split forms
//! straightforward to compare.

#![allow(missing_docs)]

use fe2o3_device::{
    Blocked, DisjointWrite, Global, Index1D, KernelContext, KernelError, KernelResult, ReadOnly,
    kernel,
};

#[allow(unused_imports)]
// Each production build selects one component from this shared source.
use crate::capability_views::{
    GlobalReadView2D, compute_attention, compute_expert_streamed, compute_parallel_router_logits,
    select_wave_top4, store_output_tile, store_packed_route,
};

#[allow(unused_imports)]
// Component features consume disjoint subsets of these shape constants.
use crate::{
    ATTENTION_OUTPUT_ELEMENTS, CONTEXT_TOKENS, EXPERT_K_TILE, EXPERT_N_TILE,
    EXPERT_OUTPUT_ELEMENTS, EXPERTS, HEAD_DIM, HIDDEN_SIZE, MATRIX_ROWS, MXFP4_BLOCKS, VALUE_TILE,
};

/// Scores 128 experts in parallel and writes a deterministic packed top-4 route.
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-gpt-oss-router-component"
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_gpt_oss_120b_router_v1(
    mut context: KernelContext<'_>,
    hidden_f32: Global<'_, f32, ReadOnly>,
    router_f32: Global<'_, f32, ReadOnly>,
    mut packed_top4: Global<'_, u32, DisjointWrite<Index1D>>,
) -> KernelResult {
    // The complete buffer contract is uniform across the launch and precedes broadcasts.
    if hidden_f32.len() < crate::PROFILE_ITEMS * HIDDEN_SIZE
        || router_f32.len() < EXPERTS * HIDDEN_SIZE
        || packed_top4.len() < crate::PACKED_ROUTE_ELEMENTS
    {
        return Err(KernelError::InvalidArgument);
    }
    // One Wave64 owns one profile item; every lane scores two expert rows.
    let index = context.invocation().index_1d();
    let global_index = index.get();
    let lane_index = global_index % crate::WAVE_SIZE;
    let item_index = global_index / crate::WAVE_SIZE;
    let [local_logit0, local_logit1] =
        compute_parallel_router_logits(&hidden_f32, &router_f32, item_index, lane_index)?;

    // Uniform broadcasts build the same tie-stable top-4 list in every lane.
    let [id0, id1, id2, id3] = select_wave_top4(&mut context, local_logit0, local_logit1);
    let packed = id0 | (id1 << 7) | (id2 << 14) | (id3 << 21);
    store_packed_route(&context, &mut packed_top4, packed)
}

/// Computes sink-stabilized BF16 attention and writes four value columns per lane.
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-gpt-oss-attention-component"
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_gpt_oss_120b_attention_v1(
    mut context: KernelContext<'_>,
    query_bf16: Global<'_, u16, ReadOnly>,
    key_transposed_bf16: Global<'_, u16, ReadOnly>,
    value_f32: Global<'_, f32, ReadOnly>,
    sinks_f32: Global<'_, f32, ReadOnly>,
    mut attention_output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
) -> KernelResult {
    // Validate all shapes before MFMA and Wave16 reduction collectives.
    if query_bf16.len() < crate::PROFILE_ITEMS * MATRIX_ROWS * HEAD_DIM
        || key_transposed_bf16.len() < crate::PROFILE_ITEMS * HEAD_DIM * CONTEXT_TOKENS
        || value_f32.len() < crate::PROFILE_ITEMS * CONTEXT_TOKENS * VALUE_TILE
        || sinks_f32.len() < crate::PROFILE_ITEMS * MATRIX_ROWS
        || attention_output.len() < ATTENTION_OUTPUT_ELEMENTS
    {
        return Err(KernelError::InvalidArgument);
    }
    // One Wave64 owns an item; its four Wave16 subgroups own four-row groups.
    let index = context.invocation().index_1d();
    let global_index = index.get();
    let lane_index = global_index % crate::WAVE_SIZE;
    let item_index = global_index / crate::WAVE_SIZE;
    // Typed MFMA views encode Q/K layout, including the host-pretransposed key stride.
    let [attention0, attention1, attention2, attention3] = compute_attention(
        &mut context,
        &query_bf16,
        &key_transposed_bf16,
        &value_f32,
        &sinks_f32,
        item_index,
        lane_index,
    )?;

    // Wave16-shaped ownership preserves the row-major 16x16 attention tile.
    store_output_tile(
        &context,
        &mut attention_output,
        [attention0, attention1, attention2, attention3],
    )
}

/// Projects through the routed MXFP4 expert selected by the materialized top-4 route.
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-gpt-oss-expert-component"
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_gpt_oss_120b_expert_v1(
    mut context: KernelContext<'_>,
    expert_activation_blocks_fp4: Global<'_, u8, ReadOnly>,
    expert_weight_blocks_fp4: Global<'_, u8, ReadOnly>,
    activation_scales: Global<'_, f32, ReadOnly>,
    expert_weight_scales: Global<'_, f32, ReadOnly>,
    packed_top4: Global<'_, u32, ReadOnly>,
    mut expert_output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
) -> KernelResult {
    // Validate the materialized boundary and all quantized operands before collectives.
    if expert_activation_blocks_fp4.len()
        < crate::PROFILE_ITEMS * MXFP4_BLOCKS * MATRIX_ROWS * EXPERT_K_TILE
        || expert_weight_blocks_fp4.len() < EXPERTS * MXFP4_BLOCKS * EXPERT_K_TILE * EXPERT_N_TILE
        || activation_scales.len() < crate::PROFILE_ITEMS * MXFP4_BLOCKS
        || expert_weight_scales.len() < EXPERTS * MXFP4_BLOCKS * EXPERT_N_TILE
        || packed_top4.len() < crate::PACKED_ROUTE_ELEMENTS
        || expert_output.len() < EXPERT_OUTPUT_ELEMENTS
    {
        return Err(KernelError::InvalidArgument);
    }
    // One Wave64 owns one item and four output values per lane.
    let index = context.invocation().index_1d();
    let global_index = index.get();
    let lane_index = global_index % crate::WAVE_SIZE;
    let item_index = global_index / crate::WAVE_SIZE;
    // Lane zero's route is broadcast so every MFMA lane selects the same expert.
    let packed_item_base = item_index.wrapping_mul(crate::WAVE_SIZE);
    let Some(packed) = GlobalReadView2D::checked(&packed_top4, packed_item_base, 1, 1, 1) else {
        return Err(KernelError::InvalidArgument);
    };
    let selected_local = (packed.load_or(0, 0, 0) as usize) & (EXPERTS - 1);
    let selected = selected_local;
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

    // Wave16-shaped ownership preserves the row-major 16x16 expert tile.
    store_output_tile(
        &context,
        &mut expert_output,
        [expert_acc0, expert_acc1, expert_acc2, expert_acc3],
    )
}
