//! Capability-based Rust kernel for dynamic strided matrix multiplication.
//!
//! Review order is deliberate: validate the dynamic contract, derive the
//! invocation's tile, materialize zero-filled lane fragments, execute one
//! convergent K-phase schedule, then apply the checked alpha/beta epilogue.

#![allow(missing_docs)] // Generated typed-kernel modules lack rustdoc in V1.

use fe2o3_device::{
    ExclusiveReadWrite, Global, KernelContext, KernelError, KernelResult, ReadOnly, StrictIeee,
    SubgroupWidth64, kernel,
};

use crate::contract::{
    FRAGMENT_ELEMENTS_PER_LANE_V1, TILE_K_V1, TILE_M_V1, TILE_N_V1, epilogue_v1, strided_extent_v1,
};

/// Exact workgroup dimensions required by the wave64 matrix profile.
pub const GENERAL_TILED_GEMM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

/// Computes `C = alpha * A * B + beta * C` for dynamic row-major matrices.
///
/// Each workgroup is one 64-lane subgroup and owns one 16x16 output tile. All
/// lanes execute the same matrix and LDS pipeline sequence; M/N/K tails become
/// positive BF16 zero before fragment formation.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        static_shared_memory_bytes = 2048
    ),
    control_flow(loop_bounds(4294967295, 4, 4))
)]
#[allow(clippy::too_many_arguments)]
pub fn tiled_gemm_general_v1(
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
    let invalid_stride = (m != 0 && k != 0 && lda < k)
        || (k != 0 && n != 0 && ldb < n)
        || (m != 0 && n != 0 && ldc < n);
    let a_extent = strided_extent_v1(m, k, lda);
    let b_extent = strided_extent_v1(k, n, ldb);
    let c_extent = strided_extent_v1(m, n, ldc);
    if invalid_stride
        || (a.len() as u64) < a_extent
        || (b.len() as u64) < b_extent
        || (c.len() as u64) < c_extent
    {
        return Err(KernelError::InvalidArgument);
    }

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

    let tile_columns = (n as usize).div_ceil(TILE_N_V1);
    if tile_columns == 0 {
        return Ok(());
    }
    let tile = invocation.workgroup_id().x() as usize;
    let tile_rows = (m as usize).div_ceil(TILE_M_V1);
    let Some(tile_count) = tile_rows.checked_mul(tile_columns) else {
        fe2o3_device::trap();
    };
    if tile >= tile_count {
        return Ok(());
    }
    let tile_row = tile / tile_columns;
    let tile_column = tile % tile_columns;
    drop(invocation);

    let mut private_accumulator = context.private_memory::<f32, 4>();
    let Some(tile_row_base) = tile_row.checked_mul(TILE_M_V1) else {
        fe2o3_device::trap();
    };
    let Some(tile_column_base) = tile_column.checked_mul(TILE_N_V1) else {
        fe2o3_device::trap();
    };
    let policy = context.numerical_policy::<StrictIeee>();
    let lane = context.subgroup_lane::<SubgroupWidth64>();
    let lane_index = lane.get() as usize;
    #[allow(deprecated)]
    let matrix = context.matrix();
    let matrix = matrix.with_numerical_policy(&policy);
    let a_matrix = matrix.bf16_a_global_row_major(&a, 0, m as usize, k as usize, lda as usize)?;
    let b_matrix = matrix.bf16_b_global_row_major(&b, 0, k as usize, n as usize, ldb as usize)?;
    let mut accumulator = matrix.bf16_zero_accumulator(&lane);
    let phase_count = (k as usize).div_ceil(TILE_K_V1);
    let mut phase = 0_usize;
    while phase < phase_count {
        let Some(reduction) = phase.checked_mul(TILE_K_V1) else {
            fe2o3_device::trap();
        };
        let a_fragment = a_matrix.load_m16k16(&lane, tile_row_base, reduction);
        let b_fragment = b_matrix.load_k16n16(&lane, reduction, tile_column_base);
        accumulator = matrix.multiply_accumulate(a_fragment, b_fragment, accumulator);
        phase += 1;
    }
    let values = accumulator.into_values();
    let mut component = 0;
    while component < FRAGMENT_ELEMENTS_PER_LANE_V1 {
        if !private_accumulator.store(component, values[component]) {
            fe2o3_device::trap();
        }
        component += 1;
    }

    let output_column = tile_column_base + lane_index % TILE_N_V1;
    let mut component = 0;
    while component < FRAGMENT_ELEMENTS_PER_LANE_V1 {
        let output_row = tile_row_base + (lane_index / TILE_N_V1) * 4 + component;
        if output_row < m as usize && output_column < n as usize {
            let Some(index) = output_row
                .checked_mul(ldc as usize)
                .and_then(|offset| offset.checked_add(output_column))
            else {
                fe2o3_device::trap();
            };
            let Some(product) = private_accumulator.load(component) else {
                fe2o3_device::trap();
            };
            let previous = c.load(index).unwrap_or(0.0);
            if !c.store(index, epilogue_v1(product, previous, alpha, beta)) {
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
        assert_eq!(strided_extent_v1(3, 2, 5), 12);
        assert_eq!(strided_extent_v1(0, 2, 5), 0);
        assert_eq!(strided_extent_v1(3, 0, 5), 0);
        assert_eq!(
            strided_extent_v1(u32::MAX, u32::MAX, u32::MAX),
            u64::from(u32::MAX) * u64::from(u32::MAX)
        );
    }
}
