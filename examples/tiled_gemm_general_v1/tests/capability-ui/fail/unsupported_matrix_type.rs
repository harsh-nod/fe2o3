use fe2o3_device::MatrixCapability;

fn unsupported_type<Brand>(matrix: &MatrixCapability<Brand>) {
    let values = [0.0_f32; 256];
    let _ = matrix.bf16_a_row_major(&values, 0, 16, 16, 16);
}
