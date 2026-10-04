use fe2o3_device::{DisjointBlock, Index1D};

fn clone_block(block: DisjointBlock<Index1D, 16, 4>) {
    let _ = block.clone();
}

fn main() {}
