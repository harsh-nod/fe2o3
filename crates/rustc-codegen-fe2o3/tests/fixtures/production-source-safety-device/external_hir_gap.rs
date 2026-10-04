#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[kernel(typed)]
pub fn external_hir_gap(input: &[u32], _output: DisjointSlice<u32>) {
    let _ = input.is_empty();
}
