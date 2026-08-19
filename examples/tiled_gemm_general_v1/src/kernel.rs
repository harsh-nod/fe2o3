//! Ordinary safe Rust for a dynamic, conservative wave64 tiled GEMM.

#![allow(missing_docs)] // Generated typed-kernel modules lack rustdoc in V1.

use fe2o3_device::{kernel, trap, DisjointSlice};
use fe2o3_gemm_device_v1::Gfx942TiledGemmWave64V1;

/// Exact workgroup dimensions required by the positive source contract.
pub const GENERAL_TILED_GEMM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Maximum number of 16-deep phases required by a `u32` K dimension.
pub const GENERAL_TILED_GEMM_MAX_PHASES_V1: u32 = 1 << 28;

// Dynamic row-major offsets are computed in u64 and converted to usize. The
// admitted gfx942 target is pointer64; fail at compile time on any other host.
const _: [(); 64] = [(); usize::BITS as usize];

fn accessed_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        return 0;
    }
    // The maximum u32 row-major extent is strictly smaller than u64::MAX.
    (u64::from(rows - 1) * u64::from(stride) + u64::from(columns)) as usize
}

fn load_bf16_or_zero(
    values: &[u16],
    row: u64,
    column: u64,
    rows: u32,
    columns: u32,
    stride: u32,
) -> u16 {
    if row >= u64::from(rows) || column >= u64::from(columns) {
        return 0;
    }
    // The bounds above reduce both coordinates to u32 domains, so this u64
    // row-major calculation cannot overflow.
    let index = row * u64::from(stride) + column;
    let Some(value) = values.get(index as usize) else {
        trap();
        return 0;
    };
    *value
}

/// Computes `C = alpha * A * B + beta * C` through 16x16x16 BF16/F32 tiles.
///
/// `A` is row-major `M x K` with row stride `lda`, `B` is row-major `K x N`
/// with row stride `ldb`, and `C` is row-major `M x N` with row stride `ldc`.
/// A 64x1x1 workgroup owns each 16x16 output tile. Every lane stages four
/// values from each operand into separate XOR4 LDS tiles, with guarded tails
/// replaced by BF16 positive zero. All lanes execute identical publish, MFMA,
/// and reuse transitions for `ceil(K / 16)` phases. The private accumulator is
/// carried across phases and the final capability performs four disjoint,
/// guarded alpha/beta stores per lane.
///
/// This is a source and type-checking contract only. Current fe2o3 compilation
/// must reject it before artifact emission because its compiler-issued safe
/// GEMM operations have no verified backend lowering yet.
#[kernel(
    typed,
    namespace = "ff5d4cbc5d1a4b23c8ecb867d4261ed7306dbf903c9c677141a50d65dac6416f",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(268435456))
)]
#[allow(clippy::too_many_arguments)]
pub fn tiled_gemm_general_v1(
    a: &[u16],
    b: &[u16],
    mut c: DisjointSlice<f32>,
    m: u32,
    n: u32,
    k: u32,
    lda: u32,
    ldb: u32,
    ldc: u32,
    alpha: f32,
    beta: f32,
) {
    let invalid_stride = (m != 0 && k != 0 && lda < k)
        || (k != 0 && n != 0 && ldb < n)
        || (m != 0 && n != 0 && ldc < n);
    let a_extent = accessed_extent(m, k, lda);
    let b_extent = accessed_extent(k, n, ldb);
    let c_extent = accessed_extent(m, n, ldc);
    if invalid_stride || a.len() < a_extent || b.len() < b_extent || c.len() < c_extent {
        trap();
        return;
    }

    let mut wave = Gfx942TiledGemmWave64V1::from_compiler(k);
    let lane = wave.lane();
    let lane_column = lane % 16;
    let lane_depth = (lane / 16) * 4;
    let a_row = u64::from(wave.tile_row()) * 16 + u64::from(lane_column);
    let b_column = u64::from(wave.tile_column()) * 16 + u64::from(lane_column);

    while wave.has_remaining_phases() {
        let depth = u64::from(wave.phase()) * 16 + u64::from(lane_depth);
        let a_bits = [
            load_bf16_or_zero(a, a_row, depth, m, k, lda),
            load_bf16_or_zero(a, a_row, depth + 1, m, k, lda),
            load_bf16_or_zero(a, a_row, depth + 2, m, k, lda),
            load_bf16_or_zero(a, a_row, depth + 3, m, k, lda),
        ];
        let b_bits = [
            load_bf16_or_zero(b, depth, b_column, k, n, ldb),
            load_bf16_or_zero(b, depth + 1, b_column, k, n, ldb),
            load_bf16_or_zero(b, depth + 2, b_column, k, n, ldb),
            load_bf16_or_zero(b, depth + 3, b_column, k, n, ldb),
        ];

        let staged = wave.stage(a_bits, b_bits);
        let published = staged.publish();
        let consumed = published.multiply_accumulate();
        wave = consumed.reuse();
    }

    wave.store_c_fragment(&mut c, m, n, ldc, alpha, beta);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extent_and_tail_loads_are_checked() {
        assert_eq!(accessed_extent(3, 2, 5), 12);
        assert_eq!(accessed_extent(0, 2, 5), 0);
        assert_eq!(accessed_extent(3, 0, 5), 0);
        let values = [0x3f80_u16, 9, 10, 11, 12, 0x4000, 19];
        assert_eq!(load_bf16_or_zero(&values, 0, 0, 2, 2, 5), 0x3f80);
        assert_eq!(load_bf16_or_zero(&values, 1, 0, 2, 2, 5), 0x4000);
        assert_eq!(load_bf16_or_zero(&values, 2, 0, 2, 2, 5), 0);
        assert_eq!(load_bf16_or_zero(&values, 0, 2, 2, 2, 5), 0);
    }
}
