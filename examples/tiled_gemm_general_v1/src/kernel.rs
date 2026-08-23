//! Safe Rust qualification kernel for dynamic strided matrix multiplication.

#![allow(missing_docs)] // Generated typed-kernel modules lack rustdoc in V1.

use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, DisjointSlice, F32AccumulatorFragment, Index1D, KernelError,
    KernelResult, Matrix, Tiled2D, Wave64, WaveLane, kernel, thread,
};

/// Exact workgroup dimensions required by the wave64 matrix profile.
pub const GENERAL_TILED_GEMM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Maximum K-loop trip count admitted by the source contract.
pub const GENERAL_TILED_GEMM_MAX_PHASES_V1: u32 = u32::MAX;

const _: [(); 64] = [(); usize::BITS as usize];

fn accessed_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        return 0;
    }
    (rows - 1) as usize * stride as usize + columns as usize
}

/// Computes `C = alpha * A * B + beta * C` for dynamic row-major matrices.
///
/// Each workgroup is one wave64 and owns one 16x16 output tile. All lanes call
/// the matrix operation uniformly; edge loads contribute BF16 zero, while the
/// checked tiled output witness suppresses stores outside logical M and N.
#[kernel(
    typed,
    namespace = "62ba887120a59fdac1b06e5e2be82d3817b557f57f282283dfcf1f957d1ec0d9",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4294967295))
)]
#[allow(clippy::too_many_arguments)]
pub fn tiled_gemm_general_v1(
    a: &[u16],
    b: &[u16],
    mut c: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
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
    let a_extent = accessed_extent(m, k, lda);
    let b_extent = accessed_extent(k, n, ldb);
    let c_extent = accessed_extent(m, n, ldc);
    if invalid_stride || a.len() < a_extent || b.len() < b_extent || c.len() < c_extent {
        return Err(KernelError::InvalidArgument);
    }

    let thread_index = thread::index_1d();
    let raw_index = thread_index.get();
    let tiles_per_row = (n as usize + 15) / 16;
    if tiles_per_row == 0 {
        return Ok(());
    }
    let tile = raw_index / 64;
    let tile_row = tile / tiles_per_row;
    let tile_column = tile % tiles_per_row;

    let output_tile = thread_index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let Ok(a_matrix) = Bf16MfmaAMatrix::row_major(
        a,
        0,
        m as usize,
        k as usize,
        lda as usize,
    ) else {
        return Err(KernelError::InvalidArgument);
    };
    let Ok(b_matrix) = Bf16MfmaBMatrix::row_major(
        b,
        0,
        k as usize,
        n as usize,
        ldb as usize,
    ) else {
        return Err(KernelError::InvalidArgument);
    };
    let wave_lane = WaveLane::<Wave64>::current();
    let matrix = Matrix::current();
    let mut accumulator = F32AccumulatorFragment::zero(&wave_lane);
    let mut phase = 0_usize;
    while phase < k as usize {
        let lhs = a_matrix.load_m16k16(&wave_lane, tile_row * 16, phase);
        let rhs = b_matrix.load_k16n16(&wave_lane, phase, tile_column * 16);
        accumulator = matrix.multiply_accumulate(lhs, rhs, accumulator);
        phase += 16;
    }

    let values = accumulator.into_values();
    if let Some(output) =
        c.get_tiled_2d_mut(&output_tile, 0, m as usize, n as usize, ldc as usize)
    {
        *output = alpha * values[0] + beta * *output;
    }
    if let Some(output) =
        c.get_tiled_2d_mut(&output_tile, 1, m as usize, n as usize, ldc as usize)
    {
        *output = alpha * values[1] + beta * *output;
    }
    if let Some(output) =
        c.get_tiled_2d_mut(&output_tile, 2, m as usize, n as usize, ldc as usize)
    {
        *output = alpha * values[2] + beta * *output;
    }
    if let Some(output) =
        c.get_tiled_2d_mut(&output_tile, 3, m as usize, n as usize, ldc as usize)
    {
        *output = alpha * values[3] + beta * *output;
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
    }
}
