#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

struct Token(u32);

#[inline(never)]
fn owned_unsafe_fn_once(seed: u32) -> u32 {
    let token = Token(seed);
    let closure = move || {
        let moved = token;
        unsafe {}
        moved.0
    };
    closure()
}

#[kernel(typed)]
pub fn unsafe_closure_reachable(seed: u32, mut output: DisjointSlice<u32>) {
    let value = owned_unsafe_fn_once(seed);
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value;
    }
}
