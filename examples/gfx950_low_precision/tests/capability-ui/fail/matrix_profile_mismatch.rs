#![forbid(unsafe_code)]

use fe2o3_device::{
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp8MfmaAFragment, Gfx950Fp8MfmaBFragment,
    Gfx950Matrix,
};

fn mixed_profile<'wave, Brand>(
    matrix: &Gfx950Matrix<Brand>,
    lhs: Gfx950Fp8MfmaAFragment<'wave, Brand>,
    rhs: Gfx950Fp8MfmaBFragment<'wave, Brand>,
    accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand>,
) {
    let _ = matrix.multiply_accumulate_fp8(lhs, rhs, accumulator);
}
