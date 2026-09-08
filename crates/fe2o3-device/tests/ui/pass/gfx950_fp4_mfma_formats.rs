use fe2o3_device::{
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaAFragment, Gfx950Fp4MfmaBFragment,
    Gfx950Fp8MfmaBFragment, PolicyGfx950Matrix, StrictIeee,
};

fn fp4_x_fp4<'wave, Brand>(
    matrix: &PolicyGfx950Matrix<'wave, Brand, Brand, StrictIeee>,
    lhs: Gfx950Fp4MfmaAFragment<'wave, Brand>,
    rhs: Gfx950Fp4MfmaBFragment<'wave, Brand>,
    accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand>,
) where
    Brand: fe2o3_device::MatrixGlobalAccess<Brand>,
{
    let _ = matrix.multiply_accumulate_fp4(lhs, rhs, accumulator);
}

fn fp4_x_fp8<'wave, Brand>(
    matrix: &PolicyGfx950Matrix<'wave, Brand, Brand, StrictIeee>,
    lhs: Gfx950Fp4MfmaAFragment<'wave, Brand>,
    rhs: Gfx950Fp8MfmaBFragment<'wave, Brand>,
    accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand>,
) where
    Brand: fe2o3_device::MatrixGlobalAccess<Brand>,
{
    let _ = matrix.multiply_accumulate_fp4_fp8(lhs, rhs, accumulator);
}

fn main() {}
