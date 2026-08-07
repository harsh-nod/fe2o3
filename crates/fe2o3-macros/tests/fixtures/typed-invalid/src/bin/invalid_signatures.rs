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
pub fn alias(a: &Floats, b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn raw_pointer(a: *const f32, b: &[f32], c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn unsupported_second(a: &[f32], b: *const f32, c: DisjointSlice<f32>) {}

#[kernel(typed)]
pub fn unsupported_third(a: &[f32], b: &[f32], c: *mut f32) {}

#[kernel(typed)]
pub fn mutable_slice(a: &mut [f32]) {}

#[kernel(typed)]
pub fn aggregate(value: (u32, u32)) {}

#[kernel(typed)]
pub fn empty() {}

fn main() {}
