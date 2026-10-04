use core::marker::PhantomData;
use fe2o3_device::{DisjointIndex, Index1D};

fn main() {
    let _ = DisjointIndex::<Index1D> {
        raw: 0,
        _index_space: PhantomData,
        _not_send_sync: PhantomData,
    };
}
