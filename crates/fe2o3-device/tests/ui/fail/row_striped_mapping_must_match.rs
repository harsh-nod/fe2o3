use fe2o3_device::{DisjointSlice, Index1D, RowStriped2D, ThreadIndex};

fn mismatched(
    mut output: DisjointSlice<u32, RowStriped2D<Index1D, 64, 4>>,
    index: ThreadIndex<Index1D>,
) {
    if let Some(stripe) = index.checked_row_striped_2d::<32, 8>() {
        let _ = output.get_row_striped_2d_mut(&stripe, 0, 3, 257, 269);
    }
}

fn main() {}
