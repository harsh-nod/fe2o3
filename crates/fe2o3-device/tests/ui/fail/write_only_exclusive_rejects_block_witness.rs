use fe2o3_device::{DisjointBlock, GridExclusive, Index1D, WriteOnlyDisjointSlice};

fn mismatch(
    output: &mut WriteOnlyDisjointSlice<u32, GridExclusive>,
    block: &DisjointBlock<Index1D, 64, 4>,
) {
    let _ = output.write_exclusive(block, 0, 1);
}

fn main() {}
