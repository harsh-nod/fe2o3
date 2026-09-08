//! Safe Rust MFMA expert projection for one dynamically selected route group.
//!
//! Review it as extent validation, wave/tile ownership, checked route and matrix
//! views, a uniform K reduction, then a gated epilogue with disjoint tiled stores.

#![allow(missing_docs)]

use fe2o3_device::{
    ExclusiveReadWrite, Global, KernelContext, KernelError, KernelResult, ReadOnly, StrictIeee,
    SubgroupWidth64, kernel,
};

pub const MOE_EXPERT_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

fn matrix_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        0
    } else {
        (rows - 1) as usize * stride as usize + columns as usize
    }
}

#[inline(always)]
fn load_2d_or<T: fe2o3_device::CapabilityMemoryElementV1, Brand>(
    values: &Global<'_, T, ReadOnly, Brand>,
    base: usize,
    row: usize,
    column: usize,
    stride: usize,
    fallback: T,
) -> T {
    row.checked_mul(stride)
        .and_then(|offset| base.checked_add(offset))
        .and_then(|offset| offset.checked_add(column))
        .and_then(|index| values.load(index))
        .unwrap_or(fallback)
}

/// Computes one routed expert group with a gated bias epilogue.
///
/// Routing packs each expert's selected tokens into a separate padded matrix.
/// The same kernel is launched for every nonempty expert; the expert argument
/// selects a strided weight and bias matrix without changing the pipeline.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4294967295, 4))
)]
#[allow(clippy::too_many_arguments)]
pub fn moe_grouped_expert_general_v1(
    context: KernelContext<'_>,
    routed_tokens: Global<'_, u16, ReadOnly>,
    expert_weights: Global<'_, u16, ReadOnly>,
    route_gates: Global<'_, f32, ReadOnly>,
    expert_bias: Global<'_, f32, ReadOnly>,
    mut routed_output: Global<'_, f32, ExclusiveReadWrite>,
    rows_padded: u32,
    output_columns: u32,
    reduction: u32,
    token_stride: u32,
    weight_stride: u32,
    expert_weight_stride: u32,
    bias_stride: u32,
    output_stride: u32,
    expert: u32,
    expert_count: u32,
) -> KernelResult {
    // Prove all dynamic extents and selected-expert offsets before MFMA.
    let token_extent = matrix_extent(rows_padded, reduction, token_stride);
    let weight_extent = if expert_count == 0 || reduction == 0 || output_columns == 0 {
        0
    } else {
        ((expert_count - 1) as usize * expert_weight_stride as usize)
            .checked_add((reduction - 1) as usize * weight_stride as usize)
            .and_then(|extent| extent.checked_add(output_columns as usize))
            .ok_or(KernelError::InvalidArgument)?
    };
    let bias_extent = matrix_extent(expert_count, output_columns, bias_stride);
    let output_extent = matrix_extent(rows_padded, output_columns, output_stride);
    if rows_padded == 0
        || !rows_padded.is_multiple_of(16)
        || output_columns == 0
        || reduction == 0
        || token_stride < reduction
        || weight_stride < output_columns
        || (expert_weight_stride as u64) < reduction as u64 * weight_stride as u64
        || bias_stride < output_columns
        || output_stride < output_columns
        || expert >= expert_count
        || routed_tokens.len() < token_extent
        || expert_weights.len() < weight_extent
        || route_gates.len() < rows_padded as usize
        || expert_bias.len() < bias_extent
        || routed_output.len() < output_extent
    {
        return Err(KernelError::InvalidArgument);
    }
    // One Wave64 owns one 16x16 routed-output tile.
    let thread_index = context.invocation().index_1d();
    let raw = thread_index.get();
    let lane = raw % 64;
    let lane_column = lane % 16;
    let tiles_per_row = ((output_columns as usize - 1) / 16) + 1;
    let tile = raw / 64;
    let tile_row = tile / tiles_per_row;
    let tile_column = tile % tiles_per_row;
    let output_column = tile_column * 16 + lane_column;
    let weight_base = expert as usize * expert_weight_stride as usize;
    let policy = context.numerical_policy::<StrictIeee>();
    let wave_lane = context.subgroup_lane::<SubgroupWidth64>();
    #[allow(deprecated)]
    let matrix = context.matrix();
    let matrix = matrix.with_numerical_policy(&policy);
    let token_matrix = matrix.bf16_a_global_row_major(
        &routed_tokens,
        0,
        rows_padded as usize,
        reduction as usize,
        token_stride as usize,
    )?;
    let weight_matrix = matrix.bf16_b_global_row_major(
        &expert_weights,
        weight_base,
        reduction as usize,
        output_columns as usize,
        weight_stride as usize,
    )?;
    // All lanes traverse the same K phases and retain accumulation in FP32.
    let mut accumulator = matrix.bf16_zero_accumulator(&wave_lane);
    let mut phase = 0_usize;
    while phase < reduction as usize {
        let lhs = token_matrix.load_m16k16(&wave_lane, tile_row * 16, phase);
        let rhs = weight_matrix.load_k16n16(&wave_lane, phase, tile_column * 16);
        accumulator = matrix.multiply_accumulate(lhs, rhs, accumulator);
        phase += 16;
    }
    let values = accumulator.into_values();

    // Fuse route gating and expert bias before capability-checked edge stores.
    let row_base = tile_row * 16 + (lane / 16) * 4;
    let bias = load_2d_or(
        &expert_bias,
        expert as usize * bias_stride as usize,
        0,
        output_column,
        bias_stride as usize,
        0.0,
    );
    let mut component = 0_usize;
    while component < values.len() {
        let row = row_base + component;
        if row < rows_padded as usize && output_column < output_columns as usize {
            let Some(index) = row
                .checked_mul(output_stride as usize)
                .and_then(|offset| offset.checked_add(output_column))
            else {
                fe2o3_device::trap();
            };
            let gate = load_2d_or(&route_gates, 0, row, 0, 1, 0.0);
            if !routed_output.store(index, gate * (values[component] + bias)) {
                fe2o3_device::trap();
            }
        }
        component += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_extent_excludes_trailing_padding() {
        assert_eq!(matrix_extent(3, 5, 8), 21);
        assert_eq!(matrix_extent(0, 5, 8), 0);
    }
}
