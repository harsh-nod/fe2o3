use fe2o3_device::{
    Bf16MfmaFragment, DeviceMatrix, F32AccumulatorFragment,
};

fn invoke(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaFragment,
    rhs: Bf16MfmaFragment,
    accumulator: F32AccumulatorFragment,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
