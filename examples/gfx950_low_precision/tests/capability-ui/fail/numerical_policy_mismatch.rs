#![forbid(unsafe_code)]

use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, MatrixCapability, NumericalPolicyCapability,
    RegisteredLaunch, StrictIeee,
};

enum MatrixBrand {}
enum PolicyBrand {}

type Root<'kernel, Kernel> =
    KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn cross_kernel_policy<'kernel>(
    matrix: &MatrixCapability<Root<'kernel, MatrixBrand>>,
    policy: &NumericalPolicyCapability<Root<'kernel, PolicyBrand>, StrictIeee>,
) {
    let _ = matrix.with_numerical_policy::<Root<'kernel, PolicyBrand>, StrictIeee>(policy);
}
