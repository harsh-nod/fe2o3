#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}
