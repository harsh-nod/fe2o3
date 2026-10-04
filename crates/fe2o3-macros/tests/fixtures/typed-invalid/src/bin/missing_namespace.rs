use gpu_device::{DisjointSlice, kernel};

#[kernel(typed)]
pub fn missing_namespace(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    let _ = (a, b, &mut c);
}

fn main() {}
