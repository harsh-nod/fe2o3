use fe2o3_device::{DisjointSlice, Index1D, Index2D, ThreadIndex};

fn matching_space(mut output: DisjointSlice<u32, Index1D>, index: ThreadIndex<Index1D>) {
    let _ = index.get();
    let _ = index.offset(1);
    let _ = index.offset_signed(-1);
    let _ = index.stride(2);
    let _ = index.stride_offset(2, 1);
    let _ = index.in_bounds(output.len());
    let _ = output.get_mut(index);
}

fn matching_2d_space(
    mut output: DisjointSlice<u32, Index2D<16>>,
    index: ThreadIndex<Index2D<16>>,
) {
    let _ = output.get_mut(index);
}

fn main() {}
