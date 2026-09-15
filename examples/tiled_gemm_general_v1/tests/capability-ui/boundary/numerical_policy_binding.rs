// expected-rejection: FE2O3-CAP-GEMM missing-policy-owner
#![no_std]

#[cfg(not(target_arch = "amdgpu"))]
compile_error!("GEMM device UI must compile the actual AMD target");

use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, MatrixCapability, RegisteredLaunch, StrictIeee,
};

enum Kernel {}
type Brand<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn missing_policy_owner<'kernel>(matrix: &MatrixCapability<Brand<'kernel>>) {
    let _ = matrix.with_numerical_policy::<Brand<'kernel>, StrictIeee>();
}
