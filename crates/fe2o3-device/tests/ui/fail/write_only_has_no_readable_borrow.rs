use fe2o3_device::{Index1D, ThreadIndex, WriteOnlyDisjointSlice};

fn read(output: &mut WriteOnlyDisjointSlice<u32>, index: ThreadIndex<Index1D>) -> u32 {
    *output.get_mut(index).unwrap()
}

fn main() {}
