use fe2o3_device::{DisjointBlock, Index1D};

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<DisjointBlock<Index1D, 16, 4>>();
}
