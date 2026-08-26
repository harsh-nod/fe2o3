use fe2o3_device::{DisjointSlice, kernel, memory, thread};

#[kernel(
    typed,
    namespace = "b3af5b6ffddbe9690db8f90eb9a3c6dcbd57ca219f23d85ec93797fd50556836"
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
