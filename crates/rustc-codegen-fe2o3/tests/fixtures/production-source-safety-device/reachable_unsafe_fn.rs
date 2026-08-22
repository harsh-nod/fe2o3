#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[inline(never)]
unsafe fn unsafe_leaf(value: u32) -> u32 {
    value.wrapping_add(1)
}

#[inline(never)]
fn safe_bridge_to_unsafe_leaf(value: u32) -> u32 {
    unsafe { unsafe_leaf(value) }
}

#[kernel(
    typed,
    namespace = "e05055c732ce1271f06624f8d47eaea1532f145d2f32e12117f962b6c4a2cccf"
)]
pub fn unsafe_reachable(_output: DisjointSlice<u32>) {
    let _ = safe_bridge_to_unsafe_leaf(17);
}
