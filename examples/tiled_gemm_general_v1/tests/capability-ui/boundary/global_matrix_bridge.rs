// expected-rejection: FE2O3-CAP-GEMM legacy-slice-global-substitution
#![no_std]

#[cfg(not(target_arch = "amdgpu"))]
compile_error!("GEMM device UI must compile the actual AMD target");

use fe2o3_device::{Global, MatrixCapability, ReadOnly};

fn missing_bridge<Brand>(
    matrix: &MatrixCapability<Brand>,
    input: &Global<'_, u16, ReadOnly, Brand>,
) {
    let _ = matrix.bf16_a_row_major(input, 0, 16, 16, 16);
}
