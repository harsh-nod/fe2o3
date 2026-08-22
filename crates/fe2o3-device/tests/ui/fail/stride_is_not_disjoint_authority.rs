use fe2o3_device::{DisjointSlice, Index1D, ThreadIndex};

fn collision_prone(mut output: DisjointSlice<u32, Index1D>, index: ThreadIndex<Index1D>) {
    let collision_prone_integer = index.stride(0);
    let _ = output.get_disjoint_mut(collision_prone_integer);
}

fn main() {}
