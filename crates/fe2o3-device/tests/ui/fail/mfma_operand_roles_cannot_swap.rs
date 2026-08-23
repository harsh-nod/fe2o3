use fe2o3_device::{
    Bf16MfmaAFragment, Bf16MfmaBFragment, DeviceMatrix,
    F32AccumulatorFragment, MfmaRowMajor,
};

fn reject_swapped_roles<'wave>(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaBFragment<'wave, MfmaRowMajor>,
    rhs: Bf16MfmaAFragment<'wave, MfmaRowMajor>,
    accumulator: F32AccumulatorFragment<'wave>,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
