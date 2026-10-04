use core::marker::PhantomData;
use fe2o3_device::{DisjointBlock, Index1D};

fn main() {
    let _ = DisjointBlock::<Index1D, 16, 4> {
        block_base: 0,
        lane: 0,
        _index_space: PhantomData,
        _not_send_sync: PhantomData,
    };
}
