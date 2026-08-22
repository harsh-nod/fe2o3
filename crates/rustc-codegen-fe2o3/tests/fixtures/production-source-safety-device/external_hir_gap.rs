#![no_std]

use fe2o3_device::{DisjointSlice, kernel};

#[kernel(
    typed,
    namespace = "e05055c732ce1271f06624f8d47eaea1532f145d2f32e12117f962b6c4a2cccf"
)]
pub fn external_hir_gap(input: &[u32], _output: DisjointSlice<u32>) {
    let _ = input.is_empty();
}
