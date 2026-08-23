use fe2o3_device::{
    Bf16F32M16N16K16, Bf16MfmaBFragment, Bf16MfmaFragment, DeviceMatrix,
    F32AccumulatorFragment, MfmaOperandA, MfmaRowMajor, Wave32,
};

fn reject_wave32<'wave>(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaFragment<
        'wave,
        MfmaOperandA,
        Bf16F32M16N16K16,
        MfmaRowMajor,
        Wave32,
    >,
    rhs: Bf16MfmaBFragment<'wave, MfmaRowMajor>,
    accumulator: F32AccumulatorFragment<'wave>,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
