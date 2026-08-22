use fe2o3_device::{Blocked, DisjointSlice, Index1D, ThreadIndex};

fn two_components(
    mut output: DisjointSlice<u32, Blocked<Index1D, 1, 2>>,
    index: ThreadIndex<Index1D>,
) {
    if let Some(block) = index.checked_block::<1, 2>() {
        if let Some(first) = output.get_block_mut(&block, 0) {
            *first = 10;
        }
        if let Some(second) = output.get_block_mut(&block, 1) {
            *second = 20;
        }
    }
}

fn zero_dimensions_issue_no_witness(index: ThreadIndex<Index1D>) {
    let _: Option<_> = index.checked_block::<0, 2>();
}

fn main() {}
