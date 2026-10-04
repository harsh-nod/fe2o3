use fe2o3_device::{
    Bf16MfmaAFragment, Bf16MfmaBFragment, DeviceMatrix, F32AccumulatorFragment,
};

fn reject_swapped_roles<'wave>(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaBFragment<'wave>,
    rhs: Bf16MfmaAFragment<'wave>,
    accumulator: F32AccumulatorFragment<'wave>,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
