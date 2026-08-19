#![forbid(unsafe_code)]
#![allow(missing_docs)]

#[path = "../positive_hostile_common.rs"]
mod positive_hostile_common;

use fe2o3_device::{DisjointSlice, kernel};
use fe2o3_gemm_device_v1::Gfx942TiledGemmWave64V1;

#[kernel(
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(2, 268435456))
)]
#[allow(clippy::too_many_arguments)]
pub fn positive_store_backedge(
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
    let a_extent = positive_hostile_common::accessed_extent(m, k, lda);
    let b_extent = positive_hostile_common::accessed_extent(k, n, ldb);
    let c_extent = positive_hostile_common::accessed_extent(m, n, ldc);
    if invalid_stride || a.len() < a_extent || b.len() < b_extent || c.len() < c_extent {
        fe2o3_device::trap();
        return;
    }

    let mut repetition = 0;
    loop {
        let mut wave = Gfx942TiledGemmWave64V1::from_compiler(k);
        let lane = wave.lane();
        let lane_column = lane % 16;
        let lane_depth = (lane / 16) * 4;
        let a_row = u64::from(wave.tile_row()) * 16 + u64::from(lane_column);
        let b_column = u64::from(wave.tile_column()) * 16 + u64::from(lane_column);
        while wave.has_remaining_phases() {
            let depth = u64::from(wave.phase()) * 16 + u64::from(lane_depth);
            let a_bits = [
                positive_hostile_common::canonical_load(a, a_row, depth, m, k, lda),
                positive_hostile_common::canonical_load(a, a_row, depth + 1, m, k, lda),
                positive_hostile_common::canonical_load(a, a_row, depth + 2, m, k, lda),
                positive_hostile_common::canonical_load(a, a_row, depth + 3, m, k, lda),
            ];
            let b_bits = [
                positive_hostile_common::canonical_load(b, depth, b_column, k, n, ldb),
                positive_hostile_common::canonical_load(b, depth + 1, b_column, k, n, ldb),
                positive_hostile_common::canonical_load(b, depth + 2, b_column, k, n, ldb),
                positive_hostile_common::canonical_load(b, depth + 3, b_column, k, n, ldb),
            ];
            let staged = wave.stage(a_bits, b_bits);
            let published = staged.publish();
            let consumed = published.multiply_accumulate();
            wave = consumed.reuse();
        }
        wave.store_c_fragment(&mut c, m, n, ldc, alpha, beta);
        repetition += 1;
        if repetition == 2 {
            break;
        }
    }
}

fn main() {}
