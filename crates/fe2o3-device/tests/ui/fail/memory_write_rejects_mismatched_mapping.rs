use fe2o3_device::{DisjointIndex, DisjointSlice, Index1D, Index2D, memory};

fn cross_mapping(
    source: &[u32],
    mut output: DisjointSlice<u32, Index1D>,
    index: DisjointIndex<Index2D<16>>,
) {
    memory::volatile_store(&mut output, &index, 7);
    memory::copy_one_nonoverlapping(source, 0, &mut output, &index);
}

fn main() {}
