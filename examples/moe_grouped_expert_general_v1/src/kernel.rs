//! Safe Rust MFMA expert projection for one dynamically selected route group.
//!
//! Review it as extent validation, wave/tile ownership, checked route and matrix
//! views, a uniform K reduction, then a gated epilogue with disjoint tiled stores.

#![allow(missing_docs)]

use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, DisjointSlice, F32AccumulatorFragment, Index1D, KernelError,
    KernelResult, Matrix, StridedReadView2D, Tiled2D, Wave64, WaveLane, kernel, thread,
};

pub const MOE_EXPERT_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

fn matrix_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        0
    } else {
        (rows - 1) as usize * stride as usize + columns as usize
    }
}

/// Computes one routed expert group with a gated bias epilogue.
///
/// Routing packs each expert's selected tokens into a separate padded matrix.
/// The same kernel is launched for every nonempty expert; the expert argument
/// selects a strided weight and bias matrix without changing the pipeline.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4294967295))
)]
#[allow(clippy::too_many_arguments)]
pub fn moe_grouped_expert_general_v1(
    routed_tokens: &[u16],
    expert_weights: &[u16],
    route_gates: &[f32],
    expert_bias: &[f32],
    mut routed_output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
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
    // Checked views separate route metadata from the quantized matrix operands.
    let gates = StridedReadView2D::from_shared_slice(route_gates, 0, rows_padded as usize, 1, 1)?;
    let biases = StridedReadView2D::from_shared_slice(
        expert_bias,
        expert as usize * bias_stride as usize,
        1,
        output_columns as usize,
        bias_stride as usize,
    )?;

    // One Wave64 owns one 16x16 routed-output tile.
    let thread_index = thread::index_1d();
    let raw = thread_index.get();
    let lane = raw % 64;
    let lane_column = lane % 16;
    let tiles_per_row = ((output_columns as usize - 1) / 16) + 1;
    let tile = raw / 64;
    let tile_row = tile / tiles_per_row;
    let tile_column = tile % tiles_per_row;
    let output_column = tile_column * 16 + lane_column;
    let output_tile = thread_index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let token_matrix = Bf16MfmaAMatrix::row_major(
        routed_tokens,
        0,
        rows_padded as usize,
        reduction as usize,
        token_stride as usize,
    )?;
    let weight_base = expert as usize * expert_weight_stride as usize;
    let weight_matrix = Bf16MfmaBMatrix::row_major(
        expert_weights,
        weight_base,
        reduction as usize,
        output_columns as usize,
        weight_stride as usize,
    )?;
    let wave_lane = WaveLane::<Wave64>::current();
    let matrix = Matrix::current();
    // All lanes traverse the same K phases and retain accumulation in FP32.
    let mut accumulator = F32AccumulatorFragment::zero(&wave_lane);
    let mut phase = 0_usize;
    while phase < reduction as usize {
        let lhs = token_matrix.load_m16k16(&wave_lane, tile_row * 16, phase);
        let rhs = weight_matrix.load_k16n16(&wave_lane, phase, tile_column * 16);
        accumulator = matrix.multiply_accumulate(lhs, rhs, accumulator);
        phase += 16;
    }

    // Fuse route gating and expert bias before capability-checked edge stores.
    let values = accumulator.into_values();
    let row_base = tile_row * 16 + (lane / 16) * 4;
    let bias = biases.load_or(0, output_column, 0.0);
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        0,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = gates.load_or(row_base, 0, 0.0) * (values[0] + bias);
    }
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        1,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = gates.load_or(row_base + 1, 0, 0.0) * (values[1] + bias);
    }
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        2,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = gates.load_or(row_base + 2, 0, 0.0) * (values[2] + bias);
    }
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        3,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = gates.load_or(row_base + 3, 0, 0.0) * (values[3] + bias);
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
