#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn library_only_export(value: u32, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = value.wrapping_add(29);
    }
}
