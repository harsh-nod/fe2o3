//! The algorithm and register map are identical for both physical B layouts.

#[cfg(feature = "bf16_mfma_column_major_b")]
use fe2o3_device::Bf16MfmaBColumnMajorMatrix;
#[cfg(all(
    feature = "bf16_mfma_row_major_b",
    not(feature = "bf16_mfma_column_major_b")
))]
use fe2o3_device::Bf16MfmaBMatrix;
use fe2o3_device::{
    Bf16MfmaAMatrix, DeviceMatrix, F32AccumulatorFragment, Index1D, Tiled2D, Wave64, WaveLane,
    WriteOnlyDisjointSlice, kernel, thread, trap,
};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1]),
    control_flow(loop_bounds(2))
)]
pub fn bf16_matrix_storage_fixture(
    left: &[u16],
    right: &[u16],
    mut output: WriteOnlyDisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
) {
    if left.len() != 5 * 32 || right.len() != 32 * 17 || output.len() != 5 * 17 {
        trap();
    }
    if thread::grid_dim_x() != 2 || thread::block_dim_x() != 64 {
        trap();
    }
    let invocation = thread::index_1d();
    let raw = invocation.get();
    let column_base = (raw / 64) * 16;
    let lane = WaveLane::<Wave64>::current();
    let Ok(left) = Bf16MfmaAMatrix::row_major(left, 0, 5, 32, 32) else {
        trap();
    };
    #[cfg(feature = "bf16_mfma_column_major_b")]
    let Ok(right) = Bf16MfmaBColumnMajorMatrix::column_major(right, 0, 32, 17, 32) else {
        trap();
    };
    #[cfg(all(
        feature = "bf16_mfma_row_major_b",
        not(feature = "bf16_mfma_column_major_b")
    ))]
    let Ok(right) = Bf16MfmaBMatrix::row_major(right, 0, 32, 17, 17) else {
        trap();
    };
    let matrix = DeviceMatrix::current();
    let mut accumulator = F32AccumulatorFragment::zero(&lane);
    let mut phase = 0_usize;
    while phase < 2 {
        let a = left.load_m16k16(&lane, 0, phase * 16);
        let b = right.load_k16n16(&lane, phase * 16, column_base);
        accumulator = matrix.multiply_accumulate(a, b, accumulator);
        phase += 1;
    }
    let [c0, c1, c2, c3] = accumulator.into_values();
    let Some(tile) = invocation.checked_tiled_2d::<64, 16, 16, 4>() else {
        trap();
    };
    let row = (raw % 64 / 16) * 4;
    let column = column_base + raw % 16;
    if row < 5 && column < 17 && !output.write_tiled_2d(&tile, 0, 5, 17, 17, c0) {
        trap();
    }
    if row + 1 < 5 && column < 17 && !output.write_tiled_2d(&tile, 1, 5, 17, 17, c1) {
        trap();
    }
    if row + 2 < 5 && column < 17 && !output.write_tiled_2d(&tile, 2, 5, 17, 17, c2) {
        trap();
    }
    if row + 3 < 5 && column < 17 && !output.write_tiled_2d(&tile, 3, 5, 17, 17, c3) {
        trap();
    }
}
