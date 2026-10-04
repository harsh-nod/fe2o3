use core::marker::PhantomData;
use fe2o3_device::{DisjointSlice, Index1D, StaticTileRegionWitness};

fn forge<'a>(
    parent: &'a mut DisjointSlice<u32>,
    ptr: *mut u32,
) -> StaticTileRegionWitness<'a, u32, Index1D, 4> {
    let _ = parent;
    StaticTileRegionWitness {
        parent_ptr: ptr,
        parent_len: 4,
        start_element: 0,
        _parent: PhantomData,
        _not_send_sync: PhantomData,
    }
}

fn main() {}
