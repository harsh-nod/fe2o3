//! Separate finite P0 fixture. This is not the existing tiled GEMM lesson.
//! No ordinary source test is allowed to infer an optimized-MIR alias/phi from
//! this spelling; the live observation must identify the actual imported shape.
#![no_std]

use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, DeviceMatrix, DisjointSlice, F32AccumulatorFragment, Index1D,
    Wave64, WaveLane, kernel, thread,
};

#[cfg(not(feature = "loop-carried"))]
#[cfg_attr(feature = "wrong-launch", kernel(typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1])))]
#[cfg_attr(not(feature = "wrong-launch"), kernel(typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])))]
pub fn tiled_region_inspection_v1(
    a: &[u16],
    b: &[u16],
    mut out: DisjointSlice<f32, Index1D>,
    selector: u32,
) {
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

    #[cfg(feature = "single-predecessor")]
    let lhs = {
        let transported = lhs;
        transported
    };

    // This has a real alternative nominal definition. Exact source admission
    // may refuse the duplicate before constructing a phi; retain that boundary.
    #[cfg(feature = "phi")]
    let lhs = if selector == 0 {
        lhs
    } else {
        a_matrix.load_m16k16(&lane, 0, 0)
    };

    // A source-level memory carrier may optimize away. The qualifier must not
    // count a retained-memory refusal unless the actual imported graph has it.
    #[cfg(feature = "retained-memory")]
    let lhs = {
        let storage = [lhs];
        let [loaded] = storage;
        loaded
    };

    let result = matrix
        .multiply_accumulate(lhs, rhs, accumulator)
        .into_values();
    // Inspecting this region does not claim a complete GEMM implementation:
    // one actual component use keeps the first source/control census small.
    if let Some(output) = out.get_mut(thread::index_1d()) {
        *output = result[0];
    }
    let _ = selector;
}

// Statement-level cfg attributes are still visible to the kernel proc-macro's
// syntactic control-flow scan. Keep the real loop in a disjoint root item, not
// hidden inside the direct root. This negative case still needs actual source
// qualification; a Rust loop spelling is not proof of a retained MIR cycle.
#[cfg(feature = "loop-carried")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
    control_flow(loop_bounds(3))
)]
pub fn tiled_region_inspection_v1(
    a: &[u16],
    b: &[u16],
    mut out: DisjointSlice<f32, Index1D>,
    selector: u32,
) {
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
    // The authored maximum is genuine: this runtime-dependent value is 0..=3.
    let mut left = selector & 3;
    while left != 0 {
        left = left.wrapping_sub(1);
        if let Some(output) = out.get_mut(thread::index_1d()) {
            *output = 0.0;
        }
    }
    let result = matrix
        .multiply_accumulate(lhs, rhs, accumulator)
        .into_values();
    if let Some(output) = out.get_mut(thread::index_1d()) {
        *output = result[0];
    }
}
