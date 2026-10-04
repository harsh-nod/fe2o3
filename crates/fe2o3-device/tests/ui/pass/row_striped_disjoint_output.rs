use fe2o3_device::{DisjointSlice, Index1D, RowStriped2D, ThreadIndex};

fn write_rows(
    mut output: DisjointSlice<u32, RowStriped2D<Index1D, 64, 4>>,
    index: ThreadIndex<Index1D>,
) {
    if let Some(stripe) = index.checked_row_striped_2d::<64, 4>() {
        for component in 0..4 {
            if let Some(element) =
                output.get_row_striped_2d_mut(&stripe, component, 3, 257, 269)
            {
                *element = component as u32;
            }
        }
    }
}

fn main() {}
