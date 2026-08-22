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
    namespace = "9168b0344c5ac9133db3b3833b6c2a1ae3b4d8cd7350381fd4682fe7fc60b9f1"
)]
pub fn unsafe_reachable(_output: DisjointSlice<u32>) {
    let _ = safe_bridge_to_unsafe_leaf(17);
}
