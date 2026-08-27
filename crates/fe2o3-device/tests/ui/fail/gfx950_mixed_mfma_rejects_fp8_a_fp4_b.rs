use fe2o3_device::{
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaBFragment,
    Gfx950Fp8MfmaAFragment, Gfx950Matrix,
};

fn reject_fp8_x_fp4<'wave>(
    matrix: &Gfx950Matrix,
    lhs: Gfx950Fp8MfmaAFragment<'wave>,
    rhs: Gfx950Fp4MfmaBFragment<'wave>,
    accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1>,
) {
    let _ = matrix.multiply_accumulate_fp4_fp8(lhs, rhs, accumulator);
}

fn main() {}
