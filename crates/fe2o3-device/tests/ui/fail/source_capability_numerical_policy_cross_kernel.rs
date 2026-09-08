use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, MatrixCapability, NumericalPolicyCapability,
    RegisteredLaunch, StrictIeee,
};

enum KernelA {}
enum KernelB {}

type BrandA<'kernel> =
    KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type BrandB<'kernel> =
    KernelCapabilityBrand<'kernel, KernelB, CurrentTarget, RegisteredLaunch>;

fn reject<'kernel>(
    matrix: &MatrixCapability<BrandA<'kernel>>,
    policy: &NumericalPolicyCapability<BrandB<'kernel>, StrictIeee>,
) {
    let _ = matrix.with_numerical_policy(policy);
}

fn main() {}
