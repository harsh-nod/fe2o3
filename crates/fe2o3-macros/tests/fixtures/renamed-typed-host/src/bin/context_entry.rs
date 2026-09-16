#![deny(warnings)]
use gpu_device::{DisjointSlice, KernelContext, kernel};

#[kernel(
    typed,
    namespace = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub fn context_entry(_context: KernelContext<'_>, _output: DisjointSlice<u32>) {}

#[kernel]
#[expect(unused_variables)]
pub fn collision(ctx: KernelContext<'_>, __fe2o3_kernel_body_collision: u32, unused: u32) {
    let _ = ctx;
}

fn main() {}
