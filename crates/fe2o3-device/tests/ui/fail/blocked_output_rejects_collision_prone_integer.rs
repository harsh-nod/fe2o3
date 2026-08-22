use fe2o3_device::{Blocked, DisjointSlice, Index1D, ThreadIndex};

fn collision_prone_mapping(
    mut output: DisjointSlice<u32, Blocked<Index1D, 16, 4>>,
    index: ThreadIndex<Index1D>,
) {
    let collision = index.stride(0);
    let _ = output.get_block_mut(&collision, 0);
}

fn main() {}
