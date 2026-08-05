use fe2o3_device::{DisjointSlice, Index1D, Index2D, ThreadIndex};

fn matching_space(mut output: DisjointSlice<u32, Index1D>, index: ThreadIndex<Index1D>) {
    let _ = output.get_mut(index);
}

fn matching_2d_space(
    mut output: DisjointSlice<u32, Index2D<16>>,
    index: ThreadIndex<Index2D<16>>,
) {
    let _ = output.get_mut(index);
}

fn main() {}
