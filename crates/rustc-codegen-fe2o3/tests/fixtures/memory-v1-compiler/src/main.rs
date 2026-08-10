use fe2o3_device::{DisjointSlice, kernel, memory};

#[kernel(
    typed,
    namespace = "8a39e9b58cfb459f4a7f1bd1a9e154388c354af66d5dab5bbf9c51fd23e60cf1"
)]
pub fn memory_v1_kernel(source: &[f32], mut destination: DisjointSlice<f32>) {
    // SAFETY: This fixture exercises the compiler contract. Runtime callers
    // must establish the documented range, alignment, and overlap obligations.
    let distance = unsafe { memory::offset_from(source, 1, 0) };
    // SAFETY: Runtime callers provide at least one initialized source element.
    let value = unsafe { memory::volatile_load(source, 0) };
    // SAFETY: Runtime callers provide one writable, disjoint destination element.
    unsafe { memory::volatile_store(&mut destination, 0, value) };
    // SAFETY: Runtime callers establish both ranges and positive-byte non-overlap.
    unsafe { memory::copy_nonoverlapping(source, 0, &mut destination, 0, 1) };
    let _ = distance;
}

fn main() {}
