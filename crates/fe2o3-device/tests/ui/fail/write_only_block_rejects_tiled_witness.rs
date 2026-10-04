use fe2o3_device::{Blocked, DisjointTile2D, Index1D, WriteOnlyDisjointSlice};

fn mismatch(
    output: &mut WriteOnlyDisjointSlice<u32, Blocked<Index1D, 64, 4>>,
    tile: &DisjointTile2D<Index1D, 64, 16, 16, 4>,
) {
    let _ = output.write_block(tile, 0, 1);
}

fn main() {}
