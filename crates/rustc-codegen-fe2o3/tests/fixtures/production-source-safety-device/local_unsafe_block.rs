#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[inline(never)]
fn local_unsafe_block() {
    unsafe {}
}

#[kernel(
    typed,
    namespace = "e05055c732ce1271f06624f8d47eaea1532f145d2f32e12117f962b6c4a2cccf"
)]
pub fn unsafe_block_reachable(_output: DisjointSlice<u32>) {
    local_unsafe_block();
}
