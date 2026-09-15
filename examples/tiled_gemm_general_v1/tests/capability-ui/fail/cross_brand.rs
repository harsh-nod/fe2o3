// expected-rejection: FE2O3-CAP-GEMM cross-kernel-matrix-lane
#![no_std]

#[cfg(not(target_arch = "amdgpu"))]
compile_error!("GEMM device UI must compile the actual AMD target");

use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, MatrixCapability, RegisteredLaunch, SubgroupLane,
    SubgroupWidth64,
};

enum KernelA {}
enum KernelB {}
type BrandA<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type BrandB<'kernel> = KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;

fn reject<'kernel>(
    matrix: &MatrixCapability<BrandA<'kernel>>,
    lane: &SubgroupLane<SubgroupWidth64, BrandB<'kernel>>,
) {
    let _ = matrix.bf16_zero_accumulator(lane);
}
