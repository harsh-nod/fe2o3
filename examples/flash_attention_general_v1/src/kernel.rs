//! Safe Rust, memory-bounded attention with MFMA score tiles.

#![allow(missing_docs)]

use fe2o3_device::{
    Bf16MfmaFragment, DisjointSlice, F32AccumulatorFragment, Index1D, KernelError, KernelResult,
    Math, Matrix, Subgroup, Tiled2D, kernel, thread,
};

pub const FLASH_ATTENTION_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
pub const FLASH_ATTENTION_MAX_KEYS_V1: u32 = 4096;
pub const FLASH_ATTENTION_MAX_DEPTH_V1: u32 = 1024;
pub const FLASH_ATTENTION_MAX_VALUE_DIMENSION_V1: u32 = 16;

fn matrix_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        0
    } else {
        (rows - 1) as usize * stride as usize + columns as usize
    }
}

/// Computes fused scaled dot-product attention without materializing scores.
///
/// Q and transposed K are BF16. V, the additive mask, and output are FP32.
/// Query rows and key columns are padded to 16; masked key padding must contain
/// negative infinity. Logical edge rows are simply ignored by the caller.
#[kernel(
    typed,
    namespace = "a3e5de83648eb444171f96f51069694e6d86ae30f8ef64e18b5cf550044ab1db",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(256, 64, 256, 64, 16))
)]
#[allow(clippy::too_many_arguments)]
pub fn flash_attention_general_v1(
    q: &[u16],
    k_transposed: &[u16],
    v: &[f32],
    additive_mask: &[f32],
    mut output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
    batch_heads: u32,
    query_rows_padded: u32,
    keys_padded: u32,
    depth: u32,
    value_dimension: u32,
    q_stride: u32,
    k_depth_stride: u32,
    k_head_stride: u32,
    v_stride: u32,
    v_head_stride: u32,
    mask_stride: u32,
    output_stride: u32,
    output_rows: u32,
    scale: f32,
) -> KernelResult {
    let expected_output_rows = batch_heads
        .checked_mul(query_rows_padded)
        .ok_or(KernelError::InvalidArgument)?;
    let q_extent = matrix_extent(output_rows, depth, q_stride);
    let k_extent = if batch_heads == 0 || depth == 0 || keys_padded == 0 {
        0
    } else {
        ((batch_heads - 1) as usize * k_head_stride as usize)
            .checked_add((depth - 1) as usize * k_depth_stride as usize)
            .and_then(|extent| extent.checked_add(keys_padded as usize))
            .ok_or(KernelError::InvalidArgument)?
    };
    let v_extent = if batch_heads == 0 || keys_padded == 0 || value_dimension == 0 {
        0
    } else {
        ((batch_heads - 1) as usize * v_head_stride as usize)
            .checked_add((keys_padded - 1) as usize * v_stride as usize)
            .and_then(|extent| extent.checked_add(value_dimension as usize))
            .ok_or(KernelError::InvalidArgument)?
    };
    let mask_extent = matrix_extent(output_rows, keys_padded, mask_stride);
    let output_extent = matrix_extent(output_rows, value_dimension, output_stride);
    if query_rows_padded == 0
        || !query_rows_padded.is_multiple_of(16)
        || keys_padded == 0
        || !keys_padded.is_multiple_of(16)
        || keys_padded > FLASH_ATTENTION_MAX_KEYS_V1
        || depth == 0
        || depth > FLASH_ATTENTION_MAX_DEPTH_V1
        || value_dimension == 0
        || value_dimension > FLASH_ATTENTION_MAX_VALUE_DIMENSION_V1
        || output_rows != expected_output_rows
        || q_stride < depth
        || k_depth_stride < keys_padded
        || (k_head_stride as u64) < depth as u64 * k_depth_stride as u64
        || v_stride < value_dimension
        || (v_head_stride as u64) < keys_padded as u64 * v_stride as u64
        || mask_stride < keys_padded
        || output_stride < value_dimension
        || q.len() < q_extent
        || k_transposed.len() < k_extent
        || v.len() < v_extent
        || additive_mask.len() < mask_extent
        || output.len() < output_extent
    {
        return Err(KernelError::InvalidArgument);
    }

    let thread_index = thread::index_1d();
    let raw = thread_index.get();
    let lane = raw % 64;
    let lane_column = lane % 16;
    let depth_offset = (lane / 16) * 4;
    let query_tile = raw / 64;
    let tiles_per_head = query_rows_padded as usize / 16;
    let head = query_tile / tiles_per_head;
    let query_row = query_tile * 16 + lane_column;
    let score_row_base = query_tile * 16 + (lane / 16) * 4;
    let output_tile = thread_index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let matrix = Matrix::current();
    let subgroup = Subgroup::current();
    let math = Math::current();

    let mut maximum0 = f32::NEG_INFINITY;
    let mut maximum1 = f32::NEG_INFINITY;
    let mut maximum2 = f32::NEG_INFINITY;
    let mut maximum3 = f32::NEG_INFINITY;
    let mut key_base = 0_usize;
    while key_base < keys_padded as usize {
        let key_column = key_base + lane_column;
        let mut scores = F32AccumulatorFragment::from_values([0.0; 4]);
        let mut phase = 0_usize;
        while phase < depth as usize {
            let d0 = phase + depth_offset;
            let d1 = d0 + 1;
            let d2 = d0 + 2;
            let d3 = d0 + 3;
            let lhs = Bf16MfmaFragment::from_bits([
                if d0 < depth as usize {
                    q[query_row * q_stride as usize + d0]
                } else {
                    0
                },
                if d1 < depth as usize {
                    q[query_row * q_stride as usize + d1]
                } else {
                    0
                },
                if d2 < depth as usize {
                    q[query_row * q_stride as usize + d2]
                } else {
                    0
                },
                if d3 < depth as usize {
                    q[query_row * q_stride as usize + d3]
                } else {
                    0
                },
            ]);
            let k_head = head * k_head_stride as usize;
            let rhs = Bf16MfmaFragment::from_bits([
                if d0 < depth as usize {
                    k_transposed[k_head + d0 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
                if d1 < depth as usize {
                    k_transposed[k_head + d1 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
                if d2 < depth as usize {
                    k_transposed[k_head + d2 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
                if d3 < depth as usize {
                    k_transposed[k_head + d3 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
            ]);
            scores = matrix.multiply_accumulate(lhs, rhs, scores);
            phase += 16;
        }
        let values = scores.into_values();
        let mask_base = score_row_base * mask_stride as usize + key_column;
        let score0 = values[0] * scale + additive_mask[mask_base];
        let score1 = values[1] * scale + additive_mask[mask_base + mask_stride as usize];
        let score2 = values[2] * scale + additive_mask[mask_base + 2 * mask_stride as usize];
        let score3 = values[3] * scale + additive_mask[mask_base + 3 * mask_stride as usize];
        if score0 > maximum0 {
            maximum0 = score0;
        }
        if score1 > maximum1 {
            maximum1 = score1;
        }
        if score2 > maximum2 {
            maximum2 = score2;
        }
        if score3 > maximum3 {
            maximum3 = score3;
        }
        key_base += 16;
    }
    maximum0 = subgroup.subgroup_reduce_max_f32::<16>(maximum0);
    maximum1 = subgroup.subgroup_reduce_max_f32::<16>(maximum1);
    maximum2 = subgroup.subgroup_reduce_max_f32::<16>(maximum2);
    maximum3 = subgroup.subgroup_reduce_max_f32::<16>(maximum3);

    let mut sum0 = 0.0_f32;
    let mut sum1 = 0.0_f32;
    let mut sum2 = 0.0_f32;
    let mut sum3 = 0.0_f32;
    let mut numerator0 = 0.0_f32;
    let mut numerator1 = 0.0_f32;
    let mut numerator2 = 0.0_f32;
    let mut numerator3 = 0.0_f32;
    key_base = 0;
    while key_base < keys_padded as usize {
        let key_column = key_base + lane_column;
        let mut scores = F32AccumulatorFragment::from_values([0.0; 4]);
        let mut phase = 0_usize;
        while phase < depth as usize {
            let d0 = phase + depth_offset;
            let d1 = d0 + 1;
            let d2 = d0 + 2;
            let d3 = d0 + 3;
            let lhs = Bf16MfmaFragment::from_bits([
                if d0 < depth as usize {
                    q[query_row * q_stride as usize + d0]
                } else {
                    0
                },
                if d1 < depth as usize {
                    q[query_row * q_stride as usize + d1]
                } else {
                    0
                },
                if d2 < depth as usize {
                    q[query_row * q_stride as usize + d2]
                } else {
                    0
                },
                if d3 < depth as usize {
                    q[query_row * q_stride as usize + d3]
                } else {
                    0
                },
            ]);
            let k_head = head * k_head_stride as usize;
            let rhs = Bf16MfmaFragment::from_bits([
                if d0 < depth as usize {
                    k_transposed[k_head + d0 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
                if d1 < depth as usize {
                    k_transposed[k_head + d1 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
                if d2 < depth as usize {
                    k_transposed[k_head + d2 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
                if d3 < depth as usize {
                    k_transposed[k_head + d3 * k_depth_stride as usize + key_column]
                } else {
                    0
                },
            ]);
            scores = matrix.multiply_accumulate(lhs, rhs, scores);
            phase += 16;
        }
        let values = scores.into_values();
        let mask_base = score_row_base * mask_stride as usize + key_column;
        let probability0 = math.exp_f32(values[0] * scale + additive_mask[mask_base] - maximum0);
        let probability1 = math.exp_f32(
            values[1] * scale + additive_mask[mask_base + mask_stride as usize] - maximum1,
        );
        let probability2 = math.exp_f32(
            values[2] * scale + additive_mask[mask_base + 2 * mask_stride as usize] - maximum2,
        );
        let probability3 = math.exp_f32(
            values[3] * scale + additive_mask[mask_base + 3 * mask_stride as usize] - maximum3,
        );
        sum0 += probability0;
        sum1 += probability1;
        sum2 += probability2;
        sum3 += probability3;

        let mut dimension = 0_usize;
        while dimension < value_dimension as usize {
            let value =
                v[head * v_head_stride as usize + key_column * v_stride as usize + dimension];
            let contribution0 = subgroup.subgroup_reduce_sum_f32::<16>(probability0 * value);
            let contribution1 = subgroup.subgroup_reduce_sum_f32::<16>(probability1 * value);
            let contribution2 = subgroup.subgroup_reduce_sum_f32::<16>(probability2 * value);
            let contribution3 = subgroup.subgroup_reduce_sum_f32::<16>(probability3 * value);
            if lane_column == dimension {
                numerator0 += contribution0;
                numerator1 += contribution1;
                numerator2 += contribution2;
                numerator3 += contribution3;
            }
            dimension += 1;
        }
        key_base += 16;
    }
    let denominator0 = subgroup.subgroup_reduce_sum_f32::<16>(sum0);
    let denominator1 = subgroup.subgroup_reduce_sum_f32::<16>(sum1);
    let denominator2 = subgroup.subgroup_reduce_sum_f32::<16>(sum2);
    let denominator3 = subgroup.subgroup_reduce_sum_f32::<16>(sum3);

    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        0,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = numerator0 / denominator0;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        1,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = numerator1 / denominator1;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        2,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = numerator2 / denominator2;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        3,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = numerator3 / denominator3;
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
