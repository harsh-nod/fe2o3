//! Mutable safe Rust candidate for the gfx942 GEMM autoresearch loop.
//!
//! Keep experiments reviewable by preserving the same phases: validate dynamic
//! extents, assign one wave per tile, construct typed matrices, execute uniform
//! K phases, and use the tiled capability for edge-safe stores.

#![allow(missing_docs)] // Generated typed-kernel modules lack rustdoc in V1.

use fe2o3_device::{
    ExclusiveReadWrite, Global, KernelContext, KernelError, KernelResult, ReadOnly, StrictIeee,
    SubgroupWidth64, kernel,
};

/// Exact workgroup dimensions required by the wave64 matrix profile.
pub const AUTORESEARCH_GEMM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

fn accessed_extent(rows: u32, columns: u32, stride: u32) -> u64 {
    if rows == 0 || columns == 0 {
        return 0;
    }
    u64::from(rows - 1) * u64::from(stride) + u64::from(columns)
}

/// Computes `C = alpha * A * B + beta * C` for dynamic row-major matrices.
///
/// Each workgroup is one wave64 and owns one 16x16 output tile. All lanes call
/// the matrix operation uniformly; edge loads contribute BF16 zero, while the
/// checked tiled output witness suppresses stores outside logical M and N.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4294967295, 4))
)]
#[allow(clippy::too_many_arguments)]
pub fn gemm_autoresearch_v1(
    context: KernelContext<'_>,
    a: Global<'_, u16, ReadOnly>,
    b: Global<'_, u16, ReadOnly>,
    mut c: Global<'_, f32, ExclusiveReadWrite>,
    m: u32,
    n: u32,
    k: u32,
    lda: u32,
    ldb: u32,
    ldc: u32,
    alpha: f32,
    beta: f32,
) -> KernelResult {
    // Reject incompatible shapes and strides before every lane enters MFMA.
    let invalid_stride = (m != 0 && k != 0 && lda < k)
        || (k != 0 && n != 0 && ldb < n)
        || (m != 0 && n != 0 && ldc < n);
    let a_extent = accessed_extent(m, k, lda);
    let b_extent = accessed_extent(k, n, ldb);
    let c_extent = accessed_extent(m, n, ldc);
    if invalid_stride
        || (a.len() as u64) < a_extent
        || (b.len() as u64) < b_extent
        || (c.len() as u64) < c_extent
    {
        return Err(KernelError::InvalidArgument);
    }

    // Grid coordinates assign one 64-lane subgroup to one 16x16 output tile.
    let invocation = context.invocation();
    let workgroup_size = invocation.workgroup_size();
    let grid_size = invocation.grid_size();
    if workgroup_size.x() != 64
        || workgroup_size.y() != 1
        || workgroup_size.z() != 1
        || grid_size.y() != 1
        || grid_size.z() != 1
    {
        fe2o3_device::trap();
    }
    let tiles_per_row = (n as usize).div_ceil(16);
    if tiles_per_row == 0 {
        return Ok(());
    }
    let tile = invocation.workgroup_id().x() as usize;
    let tile_row = tile / tiles_per_row;
    let tile_column = tile % tiles_per_row;
    let tile_rows = (m as usize).div_ceil(16);
    let Some(tile_count) = tile_rows.checked_mul(tiles_per_row) else {
        fe2o3_device::trap();
    };
    if tile >= tile_count {
        return Ok(());
    }
    drop(invocation);

    let policy = context.numerical_policy::<StrictIeee>();
    let lane = context.subgroup_lane::<SubgroupWidth64>();
    let lane_index = lane.get() as usize;
    #[allow(deprecated)]
    let matrix = context.matrix();
    let matrix = matrix.with_numerical_policy(&policy);
    let a_matrix = matrix.bf16_a_global_row_major(&a, 0, m as usize, k as usize, lda as usize)?;
    let b_matrix = matrix.bf16_b_global_row_major(&b, 0, k as usize, n as usize, ldb as usize)?;
    // All lanes traverse identical K tiles and keep the FP32 accumulator in registers.
    let mut accumulator = matrix.bf16_zero_accumulator(&lane);
    let mut phase = 0_usize;
    while phase < k as usize {
        let lhs = a_matrix.load_m16k16(&lane, tile_row * 16, phase);
        let rhs = b_matrix.load_k16n16(&lane, phase, tile_column * 16);
        accumulator = matrix.multiply_accumulate(lhs, rhs, accumulator);
        phase += 16;
    }

    // The exact lane-to-element map is checked by the final output-injectivity analysis.
    let values = accumulator.into_values();
    let output_column = tile_column * 16 + lane_index % 16;
    let mut component = 0;
    while component < 4 {
        let output_row = tile_row * 16 + (lane_index / 16) * 4 + component;
        if output_row < m as usize && output_column < n as usize {
            let Some(index) = output_row
                .checked_mul(ldc as usize)
                .and_then(|offset| offset.checked_add(output_column))
            else {
                fe2o3_device::trap();
            };
            let previous = c.load(index).unwrap_or(0.0);
            if !c.store(index, alpha * values[component] + beta * previous) {
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
    fn strided_extents_include_only_accessed_elements() {
        assert_eq!(accessed_extent(3, 2, 5), 12);
        assert_eq!(accessed_extent(0, 2, 5), 0);
        assert_eq!(accessed_extent(3, 0, 5), 0);
        assert_eq!(
            accessed_extent(u32::MAX, u32::MAX, u32::MAX),
            u64::from(u32::MAX) * u64::from(u32::MAX)
        );
    }
}
