#![forbid(unsafe_code)]

use fe2o3_device::{
    Global, MatrixCapability, MatrixGlobalAccess, NumericalPolicyCapability, ReadOnly, StrictIeee,
};

fn global_matrix<MatrixBrand, GlobalBrand>(
    matrix: &MatrixCapability<MatrixBrand>,
    policy: &NumericalPolicyCapability<GlobalBrand, StrictIeee>,
    input: &Global<'_, u8, ReadOnly, GlobalBrand>,
) where
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    let policy_matrix = matrix.with_numerical_policy(policy);
    let gfx950 = policy_matrix.gfx950();
    let _ = gfx950.fp4_a_global_row_major(input, 0, 16, 128, 128);
}
