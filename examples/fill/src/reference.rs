/// CPU result for one output coordinate; launch coverage is checked separately.
pub fn fill_reference(_point: usize, out: &mut f32) {
    *out = 42.5;
}
