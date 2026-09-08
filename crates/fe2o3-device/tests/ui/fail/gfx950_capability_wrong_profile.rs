use fe2o3_device::{
    Bf16MfmaAFragment, Bf16MfmaBFragment, F32AccumulatorFragment, MatrixGlobalAccess,
    NumericalPolicy, PolicyGfx950Matrix,
};

fn reject<'a, MatrixBrand, GlobalBrand, Policy>(
    gfx950: &PolicyGfx950Matrix<'a, MatrixBrand, GlobalBrand, Policy>,
    lhs: Bf16MfmaAFragment<'a, MatrixBrand>,
    rhs: Bf16MfmaBFragment<'a, MatrixBrand>,
    accumulator: F32AccumulatorFragment<
        'a,
        fe2o3_device::Bf16F32M16N16K16,
        fe2o3_device::MfmaAccumulatorRowMajor,
        fe2o3_device::SubgroupWidth64,
        MatrixBrand,
    >,
) where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    let _ = gfx950.multiply_accumulate_fp4(lhs, rhs, accumulator);
}

fn main() {}
