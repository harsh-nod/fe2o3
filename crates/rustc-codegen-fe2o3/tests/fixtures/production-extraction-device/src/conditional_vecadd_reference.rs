// Compiled both as authenticated reference MIR and by the host test oracle.
pub fn vecadd_reference(point: usize, a: &[f32], b: &[f32], out: &mut f32) {
    *out = a[point] + b[point];
}

pub fn wrong_index_reference(point: usize, a: &[f32], b: &[f32], out: &mut f32) {
    *out = a[point ^ 1] + b[point];
}
