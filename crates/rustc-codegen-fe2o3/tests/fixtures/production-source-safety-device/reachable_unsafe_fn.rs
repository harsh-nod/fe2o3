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
    namespace = "b14f0669db1e71dca51160be3a750616fdab9884c9a0ea17e82ac3bba346cc52"
)]
pub fn unsafe_reachable(_output: DisjointSlice<u32>) {
    let _ = safe_bridge_to_unsafe_leaf(17);
}
