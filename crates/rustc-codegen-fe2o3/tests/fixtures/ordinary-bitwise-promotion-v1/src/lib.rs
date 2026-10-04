#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

/// Ordinary Rust is the baseline; the smoke promotes only its final bitwise OR.
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn bitwise_chain(mut out: DisjointSlice<u32>, a: u32, b: u32) {
    let low = (a ^ b) & 255;
    let result = low | 256;
    let index = thread::index_1d();
    if let Some(output) = out.get_mut(index) {
        *output = result;
    }
}
