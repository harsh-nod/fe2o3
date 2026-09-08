use fe2o3_device::{
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaAFragment, Gfx950Fp8E4M3,
    Gfx950Fp8MfmaBFragment, MatrixGlobalAccess, NumericalPolicy, PolicyGfx950Matrix,
};

fn reject<'a, MatrixBrand, GlobalBrand, Policy>(
    gfx950: &PolicyGfx950Matrix<'a, MatrixBrand, GlobalBrand, Policy>,
    lhs: Gfx950Fp4MfmaAFragment<'a, MatrixBrand>,
    rhs: Gfx950Fp8MfmaBFragment<'a, MatrixBrand>,
    accumulator: Gfx950F32AccumulatorFragment<'a, Gfx950Fp4E2M1, MatrixBrand>,
) where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    let _: Gfx950F32AccumulatorFragment<'a, Gfx950Fp8E4M3, MatrixBrand> =
        gfx950.multiply_accumulate_fp8(lhs, rhs, accumulator);
}

fn main() {}
