//! Safe Rust, memory-bounded attention with MFMA score tiles.

#![allow(missing_docs)]

use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, DisjointSlice, F32AccumulatorFragment, Index1D, KernelError,
    KernelResult, Math, Matrix, StridedReadView2D, Subgroup, Tiled2D, Wave64, WaveLane, kernel,
    thread,
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
/// Query and key storage is padded to 16 for MFMA, while `query_rows` and
/// `keys` describe the independent logical extents. Padded queries and fully
/// masked logical rows produce zero; padded keys never contribute.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(256, 64, 16))
)]
#[allow(clippy::too_many_arguments)]
pub fn flash_attention_general_v1(
    q: &[u16],
    k_transposed: &[u16],
    v: &[f32],
    additive_mask: &[f32],
    mut output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
    batch_heads: u32,
    query_rows: u32,
    query_rows_padded: u32,
    keys: u32,
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
    let mask_extent = matrix_extent(output_rows, keys, mask_stride);
    let output_extent = matrix_extent(output_rows, value_dimension, output_stride);
    if batch_heads == 0
        || query_rows == 0
        || query_rows > query_rows_padded
        || query_rows_padded == 0
        || !query_rows_padded.is_multiple_of(16)
        || keys == 0
        || keys > keys_padded
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
        || mask_stride < keys
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
    let query_tile = raw / 64;
    let tiles_per_head = query_rows_padded as usize / 16;
    let expected_query_tiles = batch_heads as usize * tiles_per_head;
    if query_tile >= expected_query_tiles {
        return Err(KernelError::InvalidArgument);
    }
    let head = query_tile
        .checked_div(tiles_per_head)
        .ok_or(KernelError::InvalidArgument)?;
    let query_row_base = query_tile * 16;
    let head_row_base = head * query_rows_padded as usize;
    let score_row_base = query_tile * 16 + (lane / 16) * 4;
    // The checked head quotient makes this subtraction nonnegative.
    let score_row_in_head = score_row_base - head_row_base;
    let output_tile = thread_index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let mask = StridedReadView2D::from_shared_slice(
        additive_mask,
        head_row_base * mask_stride as usize,
        query_rows as usize,
        keys as usize,
        mask_stride as usize,
    )?;
    let v_view = StridedReadView2D::from_shared_slice(
        v,
        head * v_head_stride as usize,
        keys_padded as usize,
        value_dimension as usize,
        v_stride as usize,
    )?;
    let q_matrix = Bf16MfmaAMatrix::row_major(
        q,
        0,
        output_rows as usize,
        depth as usize,
        q_stride as usize,
    )?;
    let k_matrix = Bf16MfmaBMatrix::row_major(
        k_transposed,
        head * k_head_stride as usize,
        depth as usize,
        keys_padded as usize,
        k_depth_stride as usize,
    )?;
    let wave_lane = WaveLane::<Wave64>::current();
    let matrix = Matrix::current();
    let subgroup = Subgroup::current();
    let math = Math::current();

    let mut maximum0 = f32::NEG_INFINITY;
    let mut maximum1 = f32::NEG_INFINITY;
    let mut maximum2 = f32::NEG_INFINITY;
    let mut maximum3 = f32::NEG_INFINITY;
    let mut denominator0 = 0.0_f32;
    let mut denominator1 = 0.0_f32;
    let mut denominator2 = 0.0_f32;
    let mut denominator3 = 0.0_f32;
    let mut numerator0 = 0.0_f32;
    let mut numerator1 = 0.0_f32;
    let mut numerator2 = 0.0_f32;
    let mut numerator3 = 0.0_f32;
    let mut key_base = 0_usize;
    // Each key tile advances the stable online (maximum, sum, numerator) state.
    while key_base < keys_padded as usize {
        let key_column = key_base + lane_column;
        let mut scores = F32AccumulatorFragment::zero(&wave_lane);
        let mut phase = 0_usize;
        while phase < depth as usize {
            let lhs = q_matrix.load_m16k16(&wave_lane, query_row_base, phase);
            let rhs = k_matrix.load_k16n16(&wave_lane, phase, key_base);
            scores = matrix.multiply_accumulate(lhs, rhs, scores);
            phase += 16;
        }
        let values = scores.into_values();
        let score0 =
            values[0] * scale + mask.load_or(score_row_in_head, key_column, f32::NEG_INFINITY);
        let score1 =
            values[1] * scale + mask.load_or(score_row_in_head + 1, key_column, f32::NEG_INFINITY);
        let score2 =
            values[2] * scale + mask.load_or(score_row_in_head + 2, key_column, f32::NEG_INFINITY);
        let score3 =
            values[3] * scale + mask.load_or(score_row_in_head + 3, key_column, f32::NEG_INFINITY);
        let tile_maximum0 = subgroup.subgroup_reduce_max_f32::<16>(score0);
        let tile_maximum1 = subgroup.subgroup_reduce_max_f32::<16>(score1);
        let tile_maximum2 = subgroup.subgroup_reduce_max_f32::<16>(score2);
        let tile_maximum3 = subgroup.subgroup_reduce_max_f32::<16>(score3);
        let next_maximum0 = if tile_maximum0 > maximum0 {
            tile_maximum0
        } else {
            maximum0
        };
        let next_maximum1 = if tile_maximum1 > maximum1 {
            tile_maximum1
        } else {
            maximum1
        };
        let next_maximum2 = if tile_maximum2 > maximum2 {
            tile_maximum2
        } else {
            maximum2
        };
        let next_maximum3 = if tile_maximum3 > maximum3 {
            tile_maximum3
        } else {
            maximum3
        };
        let rescale0 = if next_maximum0 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(maximum0 - next_maximum0)
        };
        let rescale1 = if next_maximum1 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(maximum1 - next_maximum1)
        };
        let rescale2 = if next_maximum2 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(maximum2 - next_maximum2)
        };
        let rescale3 = if next_maximum3 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(maximum3 - next_maximum3)
        };
        let probability0 = if next_maximum0 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(score0 - next_maximum0)
        };
        let probability1 = if next_maximum1 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(score1 - next_maximum1)
        };
        let probability2 = if next_maximum2 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(score2 - next_maximum2)
        };
        let probability3 = if next_maximum3 == f32::NEG_INFINITY {
            0.0
        } else {
            math.exp_f32(score3 - next_maximum3)
        };
        denominator0 =
            denominator0 * rescale0 + subgroup.subgroup_reduce_sum_f32::<16>(probability0);
        denominator1 =
            denominator1 * rescale1 + subgroup.subgroup_reduce_sum_f32::<16>(probability1);
        denominator2 =
            denominator2 * rescale2 + subgroup.subgroup_reduce_sum_f32::<16>(probability2);
        denominator3 =
            denominator3 * rescale3 + subgroup.subgroup_reduce_sum_f32::<16>(probability3);
        numerator0 *= rescale0;
        numerator1 *= rescale1;
        numerator2 *= rescale2;
        numerator3 *= rescale3;

        let mut dimension = 0_usize;
        while dimension < value_dimension as usize {
            let value = v_view.load_or(key_column, dimension, 0.0);
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
        maximum0 = next_maximum0;
        maximum1 = next_maximum1;
        maximum2 = next_maximum2;
        maximum3 = next_maximum3;
        key_base += 16;
    }

    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        0,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = if denominator0 > 0.0 {
            numerator0 / denominator0
        } else {
            0.0
        };
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        1,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = if denominator1 > 0.0 {
            numerator1 / denominator1
        } else {
            0.0
        };
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        2,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = if denominator2 > 0.0 {
            numerator2 / denominator2
        } else {
            0.0
        };
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        3,
        output_rows as usize,
        value_dimension as usize,
        output_stride as usize,
    ) {
        *element = if denominator3 > 0.0 {
            numerator3 / denominator3
        } else {
            0.0
        };
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

    #[test]
    fn score_mfma_is_not_recomputed() {
        let source = include_str!("kernel.rs");
        let key_loop = ["while key_base <", " keys_padded"].concat();
        let mfma = ["matrix.multiply", "_accumulate"].concat();
        assert_eq!(source.matches(&key_loop).count(), 1);
        assert_eq!(source.matches(&mfma).count(), 1);
    }
}
