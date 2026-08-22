#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "f90f0095842089beadf9e3f52fc5e11ba8d876fa9e5860867daabc76bac2895b"
)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}
