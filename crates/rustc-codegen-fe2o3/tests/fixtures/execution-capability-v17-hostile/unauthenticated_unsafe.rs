#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[inline(never)]
unsafe fn lookalike_private_memory_from_raw_parts(value: u32) -> u32 {
    value.wrapping_add(1)
}

#[inline(never)]
fn unsafe_bridge(value: u32) -> u32 {
    unsafe { lookalike_private_memory_from_raw_parts(value) }
}

#[kernel(typed)]
pub fn unauthenticated_unsafe(_output: DisjointSlice<u32>) {
    let _ = unsafe_bridge(1);
}

