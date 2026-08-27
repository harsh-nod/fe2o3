#![allow(non_upper_case_globals)]

use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, DeviceMatrix, DisjointSlice, F32AccumulatorMatrix, Index1D,
    Tiled2D, Wave64, WaveLane, kernel, thread,
};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn tiled_gemm_v1(
    a: &[u16],
    b: &[u16],
    c: &[f32],
    mut d: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
) {
    let thread_index = thread::index_1d();
    let Some(output_tile) = thread_index.checked_tiled_2d::<64, 16, 16, 4>() else {
        return;
    };
    let lane = WaveLane::<Wave64>::current();
    let Ok(a_matrix) = Bf16MfmaAMatrix::row_major(a, 0, 16, 16, 16) else {
        fe2o3_device::trap();
        return;
    };
    let Ok(b_matrix) = Bf16MfmaBMatrix::row_major(b, 0, 16, 16, 16) else {
        fe2o3_device::trap();
        return;
    };
    let Ok(c_matrix) = F32AccumulatorMatrix::row_major(c, 0, 16, 16, 16) else {
        fe2o3_device::trap();
        return;
    };
    let lhs = a_matrix.load_m16k16(&lane, 0, 0);
    let rhs = b_matrix.load_k16n16(&lane, 0, 0);
    let accumulator = c_matrix.load_m16n16(&lane, 0, 0);
    let matrix = DeviceMatrix::current();
    let result = matrix.multiply_accumulate(lhs, rhs, accumulator).into_values();

    if let Some(output) = d.get_tiled_2d_mut(&output_tile, 0, 16, 16, 16) {
        *output = result[0];
    }
    if let Some(output) = d.get_tiled_2d_mut(&output_tile, 1, 16, 16, 16) {
        *output = result[1];
    }
    if let Some(output) = d.get_tiled_2d_mut(&output_tile, 2, 16, 16, 16) {
        *output = result[2];
    }
    if let Some(output) = d.get_tiled_2d_mut(&output_tile, 3, 16, 16, 16) {
        *output = result[3];
    }
}
