#![forbid(unsafe_code)]

use fe2o3_device::{
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaAFragment, Gfx950Fp4MfmaBFragment,
    Gfx950Matrix,
};

fn reversed_operands<'wave, Brand>(
    matrix: &Gfx950Matrix<Brand>,
    lhs: Gfx950Fp4MfmaBFragment<'wave, Brand>,
    rhs: Gfx950Fp4MfmaAFragment<'wave, Brand>,
    accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand>,
) {
    let _ = matrix.multiply_accumulate_fp4(lhs, rhs, accumulator);
}
