//! Binding-only roots: the untouched parameter is not claimed fully written.
use fe2o3_device::{DisjointSlice, KernelResult, kernel, thread};

#[kernel(typed)]
pub fn binding_first(mut output: DisjointSlice<f32>, _other: DisjointSlice<f32>) -> KernelResult {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = 42.5;
    }
    Ok(())
}

#[kernel(typed)]
pub fn binding_second(_other: DisjointSlice<f32>, mut output: DisjointSlice<f32>) -> KernelResult {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = 42.5;
    }
    Ok(())
}
