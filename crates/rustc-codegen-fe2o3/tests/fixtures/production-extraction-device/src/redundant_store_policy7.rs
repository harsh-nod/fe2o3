#![allow(unused_assignments)]
use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_store_policy7(mut output: DisjointSlice<u32>, input: u32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        let mut local = input;
        local = input;
        let reference = &mut local;
        *element = input;
        *element = *reference;
    }
}
