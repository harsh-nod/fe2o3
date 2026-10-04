use fe2o3_device::{DisjointSlice, StaticView};

fn per_invocation_partition_is_not_a_shared_slice(parent: &DisjointSlice<u32>) {
    let _ = StaticView::<u32, 4>::from_shared_slice(parent, 0);
}

fn main() {}
