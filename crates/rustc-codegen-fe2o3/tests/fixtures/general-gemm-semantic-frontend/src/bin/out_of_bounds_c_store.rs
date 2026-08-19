#![forbid(unsafe_code)]

//! Full well-typed mutation-oracle baseline for authenticated semantic diagnostics.
//! This proof-sensitive source is not the production positive kernel and cannot
//! create frontend correspondence, artifact, proof, publication, or launch authority.

use fe2o3_device::{DisjointSlice, kernel};
use fe2o3_gemm_device_v1::ProofSensitiveGeneralGemmWave64V1;

#[kernel(
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(268435456, 4))
)]
#[allow(clippy::too_many_arguments)]
pub fn valid_proof_sensitive(
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
    let mut context = ProofSensitiveGeneralGemmWave64V1::from_compiler(k);
    let lane = context.lane();
    let group_x = context.workgroup_x();
    let group_y = context.workgroup_y();
    let lane_row = lane % 16;
    let lane_column = lane % 16;
    let depth_base = 4 * (lane / 16);
    let mut accumulator0 = 0.0_f32;
    let mut accumulator1 = 0.0_f32;
    let mut accumulator2 = 0.0_f32;
    let mut accumulator3 = 0.0_f32;
    let phase_count = k / 16 + u32::from(!k.is_multiple_of(16));
    let mut phase = 0;
    while phase < phase_count {
        let mut component = 0;
        while component < 4 {
            let row = group_y * 16 + lane_row;
            let column = group_x * 16 + lane_column;
            let depth = phase * 16 + depth_base + component;
            let a_value = if row < m && depth < k {
                context.load_a(a, row, depth, m, k, lda)
            } else {
                0
            };
            let b_value = if depth < k && column < n {
                context.load_b(b, depth, column, k, n, ldb)
            } else {
                0
            };
            let tile_depth = depth_base + component;
            let a_slot = 16 * lane_row + (tile_depth ^ (4 * (lane_row % 4)));
            let b_slot = 256 + 16 * lane_column + (tile_depth ^ (4 * (lane_column % 4)));
            context.stage_value(a_slot, phase, depth, k, a_value);
            context.stage_value(b_slot, phase, depth, k, b_value);
            component += 1;
        }

        context.stage([0; 4], [0; 4]);
        context.wait_stage(phase);
        context.publish();
        let swizzled0 = depth_base ^ (4 * (lane_row % 4));
        let lhs0 = context.read_stage(16 * lane_row + swizzled0, phase);
        let rhs0 = context.read_stage(256 + 16 * lane_column + swizzled0, phase);
        accumulator0 = context.multiply_accumulate_value(lhs0, rhs0, accumulator0);
        let swizzled1 = (depth_base + 1) ^ (4 * (lane_row % 4));
        let lhs1 = context.read_stage(16 * lane_row + swizzled1, phase);
        let rhs1 = context.read_stage(256 + 16 * lane_column + swizzled1, phase);
        accumulator1 = context.multiply_accumulate_value(lhs1, rhs1, accumulator1);
        let swizzled2 = (depth_base + 2) ^ (4 * (lane_row % 4));
        let lhs2 = context.read_stage(16 * lane_row + swizzled2, phase);
        let rhs2 = context.read_stage(256 + 16 * lane_column + swizzled2, phase);
        accumulator2 = context.multiply_accumulate_value(lhs2, rhs2, accumulator2);
        let swizzled3 = (depth_base + 3) ^ (4 * (lane_row % 4));
        let lhs3 = context.read_stage(16 * lane_row + swizzled3, phase);
        let rhs3 = context.read_stage(256 + 16 * lane_column + swizzled3, phase);
        accumulator3 = context.multiply_accumulate_value(lhs3, rhs3, accumulator3);
        context.reuse();
        phase += 1;
    }

    let row_base = group_y * 16 + 4 * (lane / 16);
    let column = group_x * 16 + lane_column;
    if row_base < m && column < n {
        let initial = context.load_c(&c, row_base, column, m, n, ldc);
        let value = alpha * accumulator0 + beta * initial;
        context.store_epilogue(
            &mut c,
            m,
            column,
            m,
            n,
            ldc,
            value,
            alpha,
            accumulator0,
            beta,
            initial,
        );
    }
    if row_base + 1 < m && column < n {
        let initial = context.load_c(&c, row_base + 1, column, m, n, ldc);
        let value = alpha * accumulator1 + beta * initial;
        context.store_epilogue(
            &mut c,
            row_base + 1,
            column,
            m,
            n,
            ldc,
            value,
            alpha,
            accumulator1,
            beta,
            initial,
        );
    }
    if row_base + 2 < m && column < n {
        let initial = context.load_c(&c, row_base + 2, column, m, n, ldc);
        let value = alpha * accumulator2 + beta * initial;
        context.store_epilogue(
            &mut c,
            row_base + 2,
            column,
            m,
            n,
            ldc,
            value,
            alpha,
            accumulator2,
            beta,
            initial,
        );
    }
    if row_base + 3 < m && column < n {
        let initial = context.load_c(&c, row_base + 3, column, m, n, ldc);
        let value = alpha * accumulator3 + beta * initial;
        context.store_epilogue(
            &mut c,
            row_base + 3,
            column,
            m,
            n,
            ldc,
            value,
            alpha,
            accumulator3,
            beta,
            initial,
        );
    }
}

fn main() {}
