use fe2o3_device::{DisjointSlice, Index1D, Shifted, ThreadIndex};

fn shifted_access(
    mut output: DisjointSlice<u32, Shifted<Index1D, 1>>,
    index: ThreadIndex<Index1D>,
) {
    if let Some(index) = index.checked_shift::<1>() {
        if let Some(value) = output.get_disjoint_mut(index) {
            *value = 7;
        }
    }
}

fn main() {}
