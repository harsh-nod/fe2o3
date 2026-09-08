// expected-boundary: FE2O3-CAP-GEMM matrix views need a typed Global bridge
use fe2o3_device::{Global, MatrixCapability, ReadOnly};

fn missing_bridge<Brand>(
    matrix: &MatrixCapability<Brand>,
    input: &Global<'_, u16, ReadOnly, Brand>,
) {
    let _ = matrix.bf16_a_row_major(input, 0, 16, 16, 16);
}
