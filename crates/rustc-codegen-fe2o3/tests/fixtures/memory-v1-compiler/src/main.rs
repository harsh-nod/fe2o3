use fe2o3_device::{DisjointSlice, kernel, memory, thread};

#[kernel(
    typed,
    namespace = "8a39e9b58cfb459f4a7f1bd1a9e154388c354af66d5dab5bbf9c51fd23e60cf1"
)]
pub fn memory_v1_kernel(source: &[f32], mut destination: DisjointSlice<f32>) {
    let destination_index = thread::index_1d().into_disjoint();
    let distance = memory::offset_from(source, 1, 0);
    let value = memory::volatile_load(source, 0);
    memory::volatile_store(&mut destination, &destination_index, value);
    memory::copy_one_nonoverlapping(source, 0, &mut destination, &destination_index);
    let _ = distance;
}

fn main() {}
