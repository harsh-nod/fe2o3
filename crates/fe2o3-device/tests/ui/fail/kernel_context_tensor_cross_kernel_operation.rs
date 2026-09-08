use fe2o3_device::{
    Bf16F32M16N16K16, Bf16MfmaAFragment, Bf16MfmaBFragment, CurrentTarget, F32AccumulatorFragment,
    KernelCapabilityBrand, MatrixCapability, MfmaAccumulatorRowMajor, RegisteredLaunch, Wave64,
};

enum KernelA {}
enum KernelB {}

type BrandA<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type BrandB<'kernel> = KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;
type AccumulatorA<'wave> =
    F32AccumulatorFragment<'wave, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, BrandA<'wave>>;

fn mix_kernels<'wave>(
    matrix: &MatrixCapability<BrandA<'wave>>,
    lhs: Bf16MfmaAFragment<'wave, BrandA<'wave>>,
    rhs: Bf16MfmaBFragment<'wave, BrandB<'wave>>,
    accumulator: AccumulatorA<'wave>,
) {
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {}
