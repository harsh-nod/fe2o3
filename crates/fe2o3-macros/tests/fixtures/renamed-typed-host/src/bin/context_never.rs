#![feature(never_type)]

use gpu_device::{DisjointSlice, kernel};
type KernelContext<'a> = !;

#[kernel(
    typed,
    namespace = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub fn hidden_never(_context: KernelContext<'_>, _output: DisjointSlice<u32>) {}
fn main() {}
