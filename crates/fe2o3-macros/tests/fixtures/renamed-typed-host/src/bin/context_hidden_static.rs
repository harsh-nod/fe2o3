use gpu_device::{DisjointSlice, kernel};
type KernelContext<'a> = gpu_device::KernelContext<'static>;

#[kernel(
    typed,
    namespace = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub fn hidden_static(_context: KernelContext<'_>, _output: DisjointSlice<u32>) {}
fn main() {}
