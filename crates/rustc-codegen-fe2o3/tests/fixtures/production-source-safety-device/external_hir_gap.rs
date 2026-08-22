#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[kernel(
    typed,
    namespace = "9168b0344c5ac9133db3b3833b6c2a1ae3b4d8cd7350381fd4682fe7fc60b9f1"
)]
pub fn external_hir_gap(input: &[u32], _output: DisjointSlice<u32>) {
    let _ = input.is_empty();
}
