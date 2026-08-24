use fe2o3_device::{DisjointSlice, kernel, memory};

#[kernel(
    typed,
    namespace = "8a39e9b58cfb459f4a7f1bd1a9e154388c354af66d5dab5bbf9c51fd23e60cf1"
)]
pub fn memory_v1_kernel(source: &[f32], mut destination: DisjointSlice<f32>) {
    if source.len() < 1 || destination.len() < 1 {
        return;
    }

    let distance = memory::offset_from(source, 1, 0);
    let value = memory::volatile_load(source, 0);
    memory::volatile_store(&mut destination, 0, value);
    memory::copy_nonoverlapping(source, 0, &mut destination, 0, 1);
    let _ = distance;
}

fn main() {}
