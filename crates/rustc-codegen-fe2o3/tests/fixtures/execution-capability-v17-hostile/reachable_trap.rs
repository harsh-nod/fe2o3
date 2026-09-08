#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[inline(never)]
fn unsupported_reachable_call() {
    panic!("lookalike terminal trap must not survive collection")
}

#[kernel(typed)]
pub fn reachable_trap(_output: DisjointSlice<u32>) {
    unsupported_reachable_call();
}

