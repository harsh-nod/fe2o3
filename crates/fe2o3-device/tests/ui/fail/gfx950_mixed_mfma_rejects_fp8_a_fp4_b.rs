use fe2o3_device::{
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaBFragment, Gfx950Fp8MfmaAFragment,
    MatrixGlobalAccess, PolicyGfx950Matrix, StrictIeee,
};

fn reject_fp8_x_fp4<'wave, Brand>(
    matrix: &PolicyGfx950Matrix<'wave, Brand, Brand, StrictIeee>,
    lhs: Gfx950Fp8MfmaAFragment<'wave, Brand>,
    rhs: Gfx950Fp4MfmaBFragment<'wave, Brand>,
    accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand>,
) where
    Brand: MatrixGlobalAccess<Brand>,
{
    let _ = matrix.multiply_accumulate_fp4_fp8(lhs, rhs, accumulator);
}

fn main() {}
