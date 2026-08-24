#![allow(non_upper_case_globals)]

use fe2o3_device::{
    Bf16MfmaFragment, Blocked, DeviceMatrix, DisjointSlice, F32AccumulatorFragment, Index1D,
    kernel, thread,
};

#[kernel(
    typed,
    namespace = "7eb5edda86f1edd9b886a256243b601b8a58c48b28ac8b72ba9eb5554cdb01a8",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn tiled_gemm_v1(
    a: &[u16],
    b: &[u16],
    c: &[f32],
    mut d: DisjointSlice<f32, Blocked<Index1D, 16, 4>>,
) {
    let thread_index = thread::index_1d();
    let lane = thread_index.get();
    let lane_column = lane % 16;
    let depth_base = (lane / 16) * 4;
    let a_row_base = lane_column * 16;

    let lhs = Bf16MfmaFragment::from_bits([
        a[a_row_base + depth_base],
        a[a_row_base + depth_base + 1],
        a[a_row_base + depth_base + 2],
        a[a_row_base + depth_base + 3],
    ]);
    let rhs = Bf16MfmaFragment::from_bits([
        b[depth_base * 16 + lane_column],
        b[(depth_base + 1) * 16 + lane_column],
        b[(depth_base + 2) * 16 + lane_column],
        b[(depth_base + 3) * 16 + lane_column],
    ]);
    let accumulator = F32AccumulatorFragment::from_values([
        c[depth_base * 16 + lane_column],
        c[(depth_base + 1) * 16 + lane_column],
        c[(depth_base + 2) * 16 + lane_column],
        c[(depth_base + 3) * 16 + lane_column],
    ]);

    let matrix = DeviceMatrix::current();
    let result = matrix.multiply_accumulate(lhs, rhs, accumulator).into_values();
    let Some(output_block) = thread_index.checked_block::<16, 4>() else {
        return;
    };

    if let Some(output) = d.get_block_mut(&output_block, 0) {
        *output = result[0];
    }
    if let Some(output) = d.get_block_mut(&output_block, 1) {
        *output = result[1];
    }
    if let Some(output) = d.get_block_mut(&output_block, 2) {
        *output = result[2];
    }
    if let Some(output) = d.get_block_mut(&output_block, 3) {
        *output = result[3];
    }
}
