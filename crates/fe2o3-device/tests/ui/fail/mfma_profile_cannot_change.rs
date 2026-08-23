use fe2o3_device::{
    Bf16F32M16N16K16, Bf16MfmaBFragment, Bf16MfmaFragment, DeviceMatrix,
    F32AccumulatorFragment, MfmaOperandA, MfmaRowMajor, Wave64,
};

struct OtherProfile;

fn reject_other_profile<'wave>(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaFragment<'wave, MfmaOperandA, OtherProfile, MfmaRowMajor, Wave64>,
    rhs: Bf16MfmaBFragment<'wave, MfmaRowMajor>,
    accumulator: F32AccumulatorFragment<'wave, Bf16F32M16N16K16>,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
