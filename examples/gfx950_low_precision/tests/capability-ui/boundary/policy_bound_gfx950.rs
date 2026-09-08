#![forbid(unsafe_code)]

use fe2o3_device::{MatrixCapability, MatrixGlobalAccess, NumericalPolicyCapability, StrictIeee};

fn policy_gfx950<MatrixBrand, GlobalBrand>(
    matrix: &MatrixCapability<MatrixBrand>,
    policy: &NumericalPolicyCapability<GlobalBrand, StrictIeee>,
) where
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    let policy_matrix = matrix.with_numerical_policy(policy);
    let _ = policy_matrix.gfx950();
}
