// expected-rejection: FE2O3-CAP-GEMM cross-kernel-policy
#![no_std]

#[cfg(not(target_arch = "amdgpu"))]
compile_error!("GEMM device UI must compile the actual AMD target");

use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, MatrixCapability, NumericalPolicyCapability,
    RegisteredLaunch, StrictIeee,
};

enum KernelA {}
enum KernelB {}
type BrandA<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type BrandB<'kernel> = KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;

fn reject<'kernel>(
    matrix: &MatrixCapability<BrandA<'kernel>>,
    policy: &NumericalPolicyCapability<BrandB<'kernel>, StrictIeee>,
) {
    let _ = matrix.with_numerical_policy(policy);
}
