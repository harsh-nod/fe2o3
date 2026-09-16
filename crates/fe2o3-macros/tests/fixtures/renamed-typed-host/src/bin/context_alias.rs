use gpu_device::{DisjointSlice, kernel};
mod spelling {
    pub type KernelContext<'a> = gpu_device::KernelContext<'a>;
}

#[kernel(
    typed,
    namespace = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub fn context_alias(_context: spelling::KernelContext<'_>, _output: DisjointSlice<u32>) {}
fn main() {}
