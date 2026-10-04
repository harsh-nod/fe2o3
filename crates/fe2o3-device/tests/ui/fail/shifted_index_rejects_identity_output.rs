use fe2o3_device::{DisjointSlice, Index1D, ThreadIndex};

fn wrong_mapping(mut output: DisjointSlice<u32, Index1D>, index: ThreadIndex<Index1D>) {
    let shifted = index.checked_shift::<1>().unwrap();
    let _ = output.get_disjoint_mut(shifted);
}

fn main() {}
