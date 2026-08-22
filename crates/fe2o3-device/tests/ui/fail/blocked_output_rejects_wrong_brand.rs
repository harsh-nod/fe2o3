use fe2o3_device::{Blocked, DisjointBlock, DisjointSlice, Index1D};

fn wrong_layout(
    mut output: DisjointSlice<u32, Blocked<Index1D, 1, 2>>,
    block: &DisjointBlock<Index1D, 16, 4>,
) {
    let _ = output.get_block_mut(block, 0);
}

fn main() {}
