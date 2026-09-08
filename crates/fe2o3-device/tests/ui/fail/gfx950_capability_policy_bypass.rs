use fe2o3_device::{DeviceMath, Gfx950Matrix};

fn reject_math(math: &DeviceMath<()>) {
    let _ = math.exp_f32(1.0);
}

fn reject_matrix(matrix: &Gfx950Matrix<()>) {
    let _ = matrix.fp4_a_row_major(&[], 0, 0, 0, 0);
}

fn main() {}
