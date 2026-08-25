use fe2o3_device::{DisjointSlice, kernel, memory, thread};

#[kernel(
    typed,
    namespace = "d85329fc116f90adafc53dc05113927d687dd0474c1b7171e850641aa811ae18"
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
