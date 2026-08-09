use fe2o3_device::{DisjointSlice, StaticViewMut};

unsafe fn per_invocation_partition_is_not_global_exclusivity(
    parent: &mut DisjointSlice<u32>,
) {
    let _ = unsafe { StaticViewMut::<u32, 4>::from_globally_exclusive_slice(parent, 0) };
}

fn main() {}
