use fe2o3_device::capability_memory::{DisjointWrite, Global, ReadOnly};
use fe2o3_device::{Index1D, KernelContext, kernel};

include!("../../../../../../../examples/vecadd/src/vecadd_body.rs");

macro_rules! add_f32 {
    ($left:expr, $right:expr) => {{ $left + $right }};
}

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn vecadd(
    context: KernelContext<'_>,
    a: Global<'_, f32, ReadOnly>,
    b: Global<'_, f32, ReadOnly>,
    mut c: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let invocation = context.invocation();
    let index = invocation.index_1d();
    let _stored = vecadd_kernel_body!(@capability index, add_f32, a, b, c);
}

fn generated_arguments_use_only_three_physical_slices<'allocation>(
    a: &'allocation [f32],
    b: &'allocation [f32],
    c: &'allocation mut [f32],
) -> vecadd_gpu::Arguments<'allocation> {
    let a = fe2o3_host::__generated::GeneratedKfdReadSlice::new(a);
    let b = fe2o3_host::__generated::GeneratedKfdReadSlice::new(b);
    let c = fe2o3_host::__generated::GeneratedKfdWriteSlice::new(c);
    vecadd_gpu::Arguments::new(a, b, c)
}

fn main() {
    let _ = generated_arguments_use_only_three_physical_slices;
}
