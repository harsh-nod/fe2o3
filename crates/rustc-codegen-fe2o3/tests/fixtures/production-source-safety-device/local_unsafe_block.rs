#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[inline(never)]
fn local_unsafe_block() {
    unsafe {}
}

#[kernel(
    typed,
    namespace = "b14f0669db1e71dca51160be3a750616fdab9884c9a0ea17e82ac3bba346cc52"
)]
pub fn unsafe_block_reachable(_output: DisjointSlice<u32>) {
    local_unsafe_block();
}
