//! Safe Rust, memory-bounded attention with epoch-branded workgroup staging.
//!
//! The kernel follows five visible phases: validate shape/stride extents, map a
//! wave to a query tile, construct checked views, advance the uniform MFMA and
//! online-softmax pipeline, then store through a tiled disjoint capability.

#![allow(missing_docs)]

use fe2o3_device::{
    CapabilityMemoryElementV1, ExclusiveReadWrite, Global, KernelContext, KernelError,
    KernelResult, ReadOnly, ReusableWorkgroup, ReusableWorkgroupLds, StrictIeee, SubgroupWidth64,
    kernel,
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

#[inline(always)]
fn load_2d_or<T: CapabilityMemoryElementV1, Brand>(
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

#[inline(always)]
fn partition16_sum<'workgroup, KernelBrand>(
    phases: &mut ReusableWorkgroup<'workgroup, KernelBrand>,
    scratch: &mut ReusableWorkgroupLds<'workgroup, f32, 64, KernelBrand>,
    value: f32,
) -> f32 {
    phases.with_phase(|phase| {
        let rank = phase.invocation_rank() as usize;
        let staged = phase.bind_reusable_lds(scratch);
        let staged = staged.initialize_by_invocation(&phase, value);
        let (phase, staged) = phase.publish_lds(staged);
        let base = (rank / 16) * 16;
        let mut sum = 0.0_f32;
        let mut offset = 0_usize;
        while offset < 16 {
            sum += staged.read(&phase, base + offset).unwrap_or(0.0);
            offset += 1;
        }
        let completion = phase.finish_reusable_phase();
        (completion, sum)
    })
}

#[inline(always)]
fn partition16_max<'workgroup, KernelBrand>(
    phases: &mut ReusableWorkgroup<'workgroup, KernelBrand>,
    scratch: &mut ReusableWorkgroupLds<'workgroup, f32, 64, KernelBrand>,
    value: f32,
) -> f32 {
    phases.with_phase(|phase| {
        let rank = phase.invocation_rank() as usize;
        let staged = phase.bind_reusable_lds(scratch);
        let staged = staged.initialize_by_invocation(&phase, value);
        let (phase, staged) = phase.publish_lds(staged);
        let base = (rank / 16) * 16;
        let mut maximum = f32::NEG_INFINITY;
        let mut offset = 0_usize;
        while offset < 16 {
            let candidate = staged
                .read(&phase, base + offset)
                .unwrap_or(f32::NEG_INFINITY);
            if candidate > maximum {
                maximum = candidate;
            }
            offset += 1;
        }
        let completion = phase.finish_reusable_phase();
        (completion, maximum)
    })
}

/// Computes fused scaled dot-product attention without materializing scores.
///
/// Q and transposed K are BF16. V, the additive mask, and output are FP32.
/// Query and key storage is padded to 16 for MFMA, while `query_rows` and
/// `keys` describe the independent logical extents. Padded queries and fully
/// masked logical rows produce zero; padded keys never contribute.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        static_shared_memory_bytes = 2048
    ),
    control_flow(loop_bounds(256, 64, 16, 4))
)]
#[allow(clippy::too_many_arguments)]
pub fn flash_attention_general_v1(
    mut context: KernelContext<'_>,
    q: Global<'_, u16, ReadOnly>,
    k_transposed: Global<'_, u16, ReadOnly>,
    v: Global<'_, f32, ReadOnly>,
    additive_mask: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
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
    // Prove every dynamic extent before constructing views or entering barriers.
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

    // One Wave64 owns one 16-row query tile; Wave16 quarters own four rows each.
    let thread_index = context.invocation().index_1d();
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
    let policy = context.numerical_policy::<StrictIeee>();
    let math = context.math();
    let math = math.with_numerical_policy(&policy);

    // Keep the online-softmax maximum, denominator, and numerator in FP32.
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
    context.with_workgroup(|workgroup| -> KernelResult {
        let mut reduction_scratch = workgroup.allocate_lds::<f32, 64>().into_reusable();
        let mut phases = workgroup.into_reusable();
        let mut key_base = 0_usize;
        // Each key tile advances the stable online (maximum, sum, numerator) state.
        while key_base < keys_padded as usize {
            let key_column = key_base + lane_column;
            let values = phases.with_phase(|phase| {
                let values = {
                    let subgroup = phase.subgroup::<SubgroupWidth64>();
                    subgroup.with_matrix(phase.epoch(), |matrix, wave_lane| {
                        let matrix = matrix.with_numerical_policy(&policy);
                        let q_matrix = matrix.bf16_a_global_row_major(
                            &q,
                            0,
                            output_rows as usize,
                            depth as usize,
                            q_stride as usize,
                        )?;
                        let k_matrix = matrix.bf16_b_global_row_major(
                            &k_transposed,
                            head * k_head_stride as usize,
                            depth as usize,
                            keys_padded as usize,
                            k_depth_stride as usize,
                        )?;
                        let mut scores = matrix.bf16_zero_accumulator(wave_lane);
                        let phase_count = (depth as usize).div_ceil(16);
                        let mut phase_index = 0_usize;
                        while phase_index < phase_count {
                            let reduction_base = phase_index * 16;
                            let lhs =
                                q_matrix.load_m16k16(wave_lane, query_row_base, reduction_base);
                            let rhs = k_matrix.load_k16n16(wave_lane, reduction_base, key_base);
                            scores = matrix.multiply_accumulate(lhs, rhs, scores);
                            phase_index += 1;
                        }
                        Ok::<[f32; 4], KernelError>(scores.into_values())
                    })
                };
                let completion = phase.finish_reusable_phase();
                (completion, values)
            })?;
            let score0 = values[0] * scale
                + load_2d_or(
                    &additive_mask,
                    head_row_base * mask_stride as usize,
                    score_row_in_head,
                    key_column,
                    mask_stride as usize,
                    f32::NEG_INFINITY,
                );
            let score1 = values[1] * scale
                + load_2d_or(
                    &additive_mask,
                    head_row_base * mask_stride as usize,
                    score_row_in_head + 1,
                    key_column,
                    mask_stride as usize,
                    f32::NEG_INFINITY,
                );
            let score2 = values[2] * scale
                + load_2d_or(
                    &additive_mask,
                    head_row_base * mask_stride as usize,
                    score_row_in_head + 2,
                    key_column,
                    mask_stride as usize,
                    f32::NEG_INFINITY,
                );
            let score3 = values[3] * scale
                + load_2d_or(
                    &additive_mask,
                    head_row_base * mask_stride as usize,
                    score_row_in_head + 3,
                    key_column,
                    mask_stride as usize,
                    f32::NEG_INFINITY,
                );
            let tile_maximum0 = partition16_max(&mut phases, &mut reduction_scratch, score0);
            let tile_maximum1 = partition16_max(&mut phases, &mut reduction_scratch, score1);
            let tile_maximum2 = partition16_max(&mut phases, &mut reduction_scratch, score2);
            let tile_maximum3 = partition16_max(&mut phases, &mut reduction_scratch, score3);
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
            denominator0 = denominator0 * rescale0
                + partition16_sum(&mut phases, &mut reduction_scratch, probability0);
            denominator1 = denominator1 * rescale1
                + partition16_sum(&mut phases, &mut reduction_scratch, probability1);
            denominator2 = denominator2 * rescale2
                + partition16_sum(&mut phases, &mut reduction_scratch, probability2);
            denominator3 = denominator3 * rescale3
                + partition16_sum(&mut phases, &mut reduction_scratch, probability3);
            numerator0 *= rescale0;
            numerator1 *= rescale1;
            numerator2 *= rescale2;
            numerator3 *= rescale3;

            let mut dimension = 0_usize;
            while dimension < value_dimension as usize {
                let value = load_2d_or(
                    &v,
                    head * v_head_stride as usize,
                    key_column,
                    dimension,
                    v_stride as usize,
                    0.0,
                );
                let contribution0 =
                    partition16_sum(&mut phases, &mut reduction_scratch, probability0 * value);
                let contribution1 =
                    partition16_sum(&mut phases, &mut reduction_scratch, probability1 * value);
                let contribution2 =
                    partition16_sum(&mut phases, &mut reduction_scratch, probability2 * value);
                let contribution3 =
                    partition16_sum(&mut phases, &mut reduction_scratch, probability3 * value);
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

        // The lane/tile mapping is injective; final-graph ownership analysis
        // must prove it before admitting these exclusive-allocation stores.
        let numerators = [numerator0, numerator1, numerator2, numerator3];
        let denominators = [denominator0, denominator1, denominator2, denominator3];
        let mut component = 0_usize;
        while component < numerators.len() {
            let row = score_row_base + component;
            if row < output_rows as usize && lane_column < value_dimension as usize {
                let Some(index) = row
                    .checked_mul(output_stride as usize)
                    .and_then(|offset| offset.checked_add(lane_column))
                else {
                    fe2o3_device::trap();
                };
                let value = if denominators[component] > 0.0 {
                    numerators[component] / denominators[component]
                } else {
                    0.0
                };
                if !output.store(index, value) {
                    fe2o3_device::trap();
                }
            }
            component += 1;
        }
        Ok(())
    })
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
    fn score_mfma_and_softmax_use_branded_workgroup_phases() {
        let source = include_str!("kernel.rs");
        let key_loop = ["while key_base <", " keys_padded"].concat();
        let mfma = ["matrix.multiply", "_accumulate"].concat();
        assert_eq!(source.matches(&key_loop).count(), 1);
        assert_eq!(source.matches(&mfma).count(), 1);
        assert!(source.contains("context.with_workgroup"));
        assert!(source.contains("workgroup.allocate_lds::<f32, 64>()"));
        assert!(source.contains("phases.with_phase"));
        assert!(source.contains("phase.publish_lds"));
        assert!(source.contains("phase.finish_reusable_phase"));
    }
}
