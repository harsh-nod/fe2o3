use fe2o3_device::{DisjointBlock, Index1D};

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<DisjointBlock<Index1D, 16, 4>>();
}
