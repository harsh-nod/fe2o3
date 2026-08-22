#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[kernel(
    typed,
    namespace = "b14f0669db1e71dca51160be3a750616fdab9884c9a0ea17e82ac3bba346cc52"
)]
pub fn external_hir_gap(input: &[u32], _output: DisjointSlice<u32>) {
    let _ = input.is_empty();
}
