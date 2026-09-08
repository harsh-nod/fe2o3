use fe2o3_device::MatrixCapability;

fn bypass_checked_extents<Brand>(matrix: &MatrixCapability<Brand>, values: &[u16]) {
    let _ = matrix.bf16_a_row_major_unchecked(values, 0, 16, 16, 8);
}
