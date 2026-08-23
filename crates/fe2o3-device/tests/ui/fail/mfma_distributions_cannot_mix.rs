use fe2o3_device::{
    Bf16MfmaAFragment, Bf16MfmaBFragment, DeviceMatrix,
    F32AccumulatorFragment, MfmaRowMajor, MfmaRowMajorXor4,
};

fn reject_mixed_distributions<'wave>(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaAFragment<'wave, MfmaRowMajor>,
    rhs: Bf16MfmaBFragment<'wave, MfmaRowMajorXor4>,
    accumulator: F32AccumulatorFragment<'wave>,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
