//! Safe Rust MFMA expert projection for one dynamically selected route group.

#![allow(missing_docs)]

use fe2o3_device::{
    Bf16MfmaFragment, DeviceMatrix, DisjointSlice, F32AccumulatorFragment, Index1D, Tiled2D,
    kernel, thread,
};

pub const MOE_EXPERT_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
pub const MOE_MAX_REDUCTION_PHASES_V1: u32 = u32::MAX;

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
    namespace = "89e87792d88730af30400017f22a90b9e55bf591184188cf9c6d7de807424ae1",
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
) {
    let token_extent = matrix_extent(rows_padded, reduction, token_stride);
    let weight_extent = if expert_count == 0 || reduction == 0 || output_columns == 0 {
        0
    } else {
        (expert_count - 1) as usize * expert_weight_stride as usize
            + (reduction - 1) as usize * weight_stride as usize
            + output_columns as usize
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
        return;
    }

    let thread_index = thread::index_1d();
    let raw = thread_index.get();
    let lane = raw % 64;
    let lane_column = lane % 16;
    let depth_offset = (lane / 16) * 4;
    let tiles_per_row = (output_columns as usize + 15) / 16;
    let tile = raw / 64;
    let tile_row = tile / tiles_per_row;
    let tile_column = tile % tiles_per_row;
    let token_row = tile_row * 16 + lane_column;
    let output_column = tile_column * 16 + lane_column;
    let Some(output_tile) = thread_index.checked_tiled_2d::<64, 16, 16, 4>() else {
        return;
    };
    let matrix = DeviceMatrix::current();
    let mut accumulator = F32AccumulatorFragment::from_values([0.0; 4]);
    let mut phase = 0_usize;
    while phase < reduction as usize {
        let d0 = phase + depth_offset;
        let d1 = d0 + 1;
        let d2 = d0 + 2;
        let d3 = d0 + 3;
        let lhs = Bf16MfmaFragment::from_bits([
            if d0 < reduction as usize {
                routed_tokens[token_row * token_stride as usize + d0]
            } else {
                0
            },
            if d1 < reduction as usize {
                routed_tokens[token_row * token_stride as usize + d1]
            } else {
                0
            },
            if d2 < reduction as usize {
                routed_tokens[token_row * token_stride as usize + d2]
            } else {
                0
            },
            if d3 < reduction as usize {
                routed_tokens[token_row * token_stride as usize + d3]
            } else {
                0
            },
        ]);
        let weight_base = expert as usize * expert_weight_stride as usize;
        let rhs = Bf16MfmaFragment::from_bits([
            if d0 < reduction as usize && output_column < output_columns as usize {
                expert_weights[weight_base + d0 * weight_stride as usize + output_column]
            } else {
                0
            },
            if d1 < reduction as usize && output_column < output_columns as usize {
                expert_weights[weight_base + d1 * weight_stride as usize + output_column]
            } else {
                0
            },
            if d2 < reduction as usize && output_column < output_columns as usize {
                expert_weights[weight_base + d2 * weight_stride as usize + output_column]
            } else {
                0
            },
            if d3 < reduction as usize && output_column < output_columns as usize {
                expert_weights[weight_base + d3 * weight_stride as usize + output_column]
            } else {
                0
            },
        ]);
        accumulator = matrix.multiply_accumulate(lhs, rhs, accumulator);
        phase += 16;
    }

    let values = accumulator.into_values();
    let row_base = tile_row * 16 + (lane / 16) * 4;
    let bias = if output_column < output_columns as usize {
        expert_bias[expert as usize * bias_stride as usize + output_column]
    } else {
        0.0
    };
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        0,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = route_gates[row_base] * (values[0] + bias);
    }
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        1,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = route_gates[row_base + 1] * (values[1] + bias);
    }
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        2,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = route_gates[row_base + 2] * (values[2] + bias);
    }
    if let Some(element) = routed_output.get_tiled_2d_mut(
        &output_tile,
        3,
        rows_padded as usize,
        output_columns as usize,
        output_stride as usize,
    ) {
        *element = route_gates[row_base + 3] * (values[3] + bias);
    }
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
