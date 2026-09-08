#![forbid(unsafe_code)]

use fe2o3_device::{MatrixCapability, NumericalPolicyCapability, StrictIeee};

fn cross_kernel_policy<MatrixBrand, PolicyBrand>(
    matrix: &MatrixCapability<MatrixBrand>,
    policy: &NumericalPolicyCapability<PolicyBrand, StrictIeee>,
) {
    let _ = matrix.with_numerical_policy(policy);
}
