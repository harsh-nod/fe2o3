#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[inline(never)]
fn local_unsafe_block() {
    unsafe {}
}

#[kernel(
    typed,
    namespace = "9168b0344c5ac9133db3b3833b6c2a1ae3b4d8cd7350381fd4682fe7fc60b9f1"
)]
pub fn unsafe_block_reachable(_output: DisjointSlice<u32>) {
    local_unsafe_block();
}
