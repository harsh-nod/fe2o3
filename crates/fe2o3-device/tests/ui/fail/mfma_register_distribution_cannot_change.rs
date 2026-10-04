use fe2o3_device::{
    Bf16F32M16N16K16, Bf16MfmaBFragment, Bf16MfmaFragment, DeviceMatrix,
    F32AccumulatorFragment, MfmaOperandA, Wave64,
};

struct OtherRegisterDistribution;

fn reject_other_register_distribution<'wave>(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaFragment<
        'wave,
        MfmaOperandA,
        Bf16F32M16N16K16,
        OtherRegisterDistribution,
        Wave64,
    >,
    rhs: Bf16MfmaBFragment<'wave>,
    accumulator: F32AccumulatorFragment<'wave>,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
