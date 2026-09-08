use fe2o3_device::{
    CurrentTarget, Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaAFragment,
    Gfx950Fp4MfmaBFragment, KernelCapabilityBrand, PolicyGfx950Matrix, RegisteredLaunch,
    StrictIeee,
};

enum KernelA {}
enum KernelB {}

type BrandA<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type BrandB<'kernel> = KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;

fn mix_gfx950_kernels<'wave>(
    matrix: &PolicyGfx950Matrix<'wave, BrandA<'wave>, BrandA<'wave>, StrictIeee>,
    lhs: Gfx950Fp4MfmaAFragment<'wave, BrandA<'wave>>,
    rhs: Gfx950Fp4MfmaBFragment<'wave, BrandB<'wave>>,
    accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, BrandA<'wave>>,
) {
    let _ = matrix.multiply_accumulate_fp4(lhs, rhs, accumulator);
}

fn main() {}
