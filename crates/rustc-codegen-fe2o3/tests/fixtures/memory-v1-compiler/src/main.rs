use fe2o3_device::{DisjointSlice, kernel, memory};

#[kernel(
    typed,
    namespace = "8a39e9b58cfb459f4a7f1bd1a9e154388c354af66d5dab5bbf9c51fd23e60cf1"
)]
pub fn memory_v1_kernel(source: &[f32], mut destination: DisjointSlice<f32>) {
    memory_v1_checked(source, &mut destination);
}

#[inline(never)]
fn memory_v1_checked(source: &[f32], destination: &mut DisjointSlice<f32>) {
    if source.is_empty() || destination.is_empty() {
        return;
    }

    // SAFETY: This fixture exercises the compiler contract. Runtime callers
    // provide initialized source storage. The checks above establish both
    // one-element ranges, while DisjointSlice construction excludes aliases.
    let distance = unsafe { memory::offset_from(source, 1, 0) };
    // SAFETY: The source is initialized and the checked index is in bounds.
    let value = unsafe { memory::volatile_load(source, 0) };
    // SAFETY: The checked destination is writable and exclusively owned.
    unsafe { memory::volatile_store(destination, 0, value) };
    // SAFETY: Both checked one-element ranges are valid and DisjointSlice's
    // construction contract excludes positive-byte overlap with source.
    unsafe { memory::copy_nonoverlapping(source, 0, destination, 0, 1) };
    let _ = distance;
}

fn main() {}
