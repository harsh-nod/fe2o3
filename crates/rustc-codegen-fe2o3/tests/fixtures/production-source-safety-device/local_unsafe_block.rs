#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[inline(never)]
fn local_unsafe_block() {
    unsafe {}
}

#[kernel(typed)]
pub fn unsafe_block_reachable(_output: DisjointSlice<u32>) {
    local_unsafe_block();
}
