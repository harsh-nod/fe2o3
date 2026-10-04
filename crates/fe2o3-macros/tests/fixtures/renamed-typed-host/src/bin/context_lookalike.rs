use gpu_device::{DisjointSlice, kernel};
pub struct KernelContext<'a>(core::marker::PhantomData<&'a ()>);

#[kernel(
    typed,
    namespace = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub fn lookalike(_context: KernelContext<'_>, _output: DisjointSlice<u32>) {}
fn main() {}
