use gpu_device::{DisjointSlice, kernel};

type Floats = [f32];

#[kernel(typed)]
fn private(a: &[f32], b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub unsafe fn unsafe_kernel(a: &[f32], b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn generic<T>(a: &[f32], b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn result(a: &[f32], b: &[f32], c: DisjointSlice<f32>) -> Result<(), ()> {
    Ok(())
}

#[kernel(typed)]
pub fn wrong_count(a: &[f32], b: &[f32]) {}

#[kernel(typed)]
pub fn alias(a: &Floats, b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn wrong_element(a: &[u32], b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn wrong_order(a: &[f32], b: DisjointSlice<f32>, c: &[f32]) {}

#[kernel(typed)]
pub fn raw_pointer(a: *const f32, b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn wrong_output(a: &[f32], b: &[f32], c: DisjointSlice<u32>) {}

fn main() {}
