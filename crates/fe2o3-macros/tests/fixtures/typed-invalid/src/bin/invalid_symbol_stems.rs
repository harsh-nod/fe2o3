use gpu_device::{DisjointSlice, kernel};

#[kernel(typed)]
pub fn r#type(a: &[f32], b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(
    a: &[f32],
    b: &[f32],
    c: DisjointSlice<f32>,
) {
}

fn main() {}
