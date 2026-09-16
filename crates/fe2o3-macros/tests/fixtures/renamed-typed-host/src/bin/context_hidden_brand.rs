use gpu_device::{DisjointSlice, kernel};
pub enum OtherBrand {}
type KernelContext<'a> = gpu_device::KernelContext<'a, OtherBrand>;

#[kernel(
    typed,
    namespace = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub fn hidden_brand(_context: KernelContext<'_>, _output: DisjointSlice<u32>) {}
fn main() {}
