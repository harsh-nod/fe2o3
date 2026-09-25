//! Hand-authored Increment I fixture: one real helper, not generated source.
//! Checked transport is not ordinary materialization, numerical CPU or GPU evidence.
#![no_std]
use fe2o3_device::{
    Bf16MfmaAFragment, Bf16MfmaAMatrix, Bf16MfmaBFragment, Bf16MfmaBMatrix, DeviceMatrix,
    DisjointSlice, F32AccumulatorFragment, Index1D, Wave64, WaveLane, kernel, thread,
};
#[cfg_attr(feature = "wrong-launch", kernel(typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1])))]
#[cfg_attr(not(feature = "wrong-launch"), kernel(typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])))]
pub fn bf16_tile_promotion_v1(
    a: &[u16],
    b: &[u16],
    mut out: DisjointSlice<f32, Index1D>,
    selector: u32,
) {
    #[inline(never)]
    fn __fe2o3_bf16_tile<'wave>(
        matrix: &DeviceMatrix,
        lhs: Bf16MfmaAFragment<'wave>,
        rhs: Bf16MfmaBFragment<'wave>,
        accumulator: F32AccumulatorFragment<'wave>,
    ) -> [f32; 4] {
        let values = matrix
            .multiply_accumulate(lhs, rhs, accumulator)
            .into_values();
        #[cfg(feature = "swap01")]
        {
            [values[1], values[0], values[2], values[3]]
        }
        #[cfg(not(feature = "swap01"))]
        {
            [values[0], values[1], values[2], values[3]]
        }
    }
    let lane = WaveLane::<Wave64>::current();
    let Ok(a_matrix) = Bf16MfmaAMatrix::row_major(a, 0, 16, 16, 16) else {
        fe2o3_device::trap();
    };
    let Ok(b_matrix) = Bf16MfmaBMatrix::row_major(b, 0, 16, 16, 16) else {
        fe2o3_device::trap();
    };
    let lhs = a_matrix.load_m16k16(&lane, 0, 0);
    let rhs = b_matrix.load_k16n16(&lane, 0, 0);
    let accumulator = F32AccumulatorFragment::zero(&lane);
    let matrix = DeviceMatrix::current();
    let result = __fe2o3_bf16_tile(&matrix, lhs, rhs, accumulator);
    if let Some(output) = out.get_mut(thread::index_1d()) {
        *output = result[0];
    }
    let _ = selector;
}
