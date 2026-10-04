use fe2o3_device::{DisjointBlock, Index1D};

fn duplicate(block: DisjointBlock<Index1D, 16, 4>) {
    let _first = block;
    let _second = block;
}

fn main() {}
