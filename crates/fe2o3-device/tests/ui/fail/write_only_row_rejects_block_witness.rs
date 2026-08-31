use fe2o3_device::{DisjointBlock, Index1D, RowStriped2D, WriteOnlyDisjointSlice};

fn mismatch(
    output: &mut WriteOnlyDisjointSlice<u32, RowStriped2D<Index1D, 16, 4>>,
    block: &DisjointBlock<Index1D, 16, 4>,
) {
    let _ = output.write_row_striped_2d(block, 0, 1, 1, 1, 1);
}

fn main() {}
