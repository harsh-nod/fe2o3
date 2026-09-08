use fe2o3_device::capability_memory::{DisjointWrite, Global, ReadOnly};
use fe2o3_device::{Index1D, KernelContext, kernel};

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
    let index = context.invocation().index_1d();
    let i = index.get();
    let _ = c.store(
        index.into_disjoint(),
        a.load(i).unwrap_or(0.0) + b.load(i).unwrap_or(0.0),
    );
}

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn copy(
    context: KernelContext<'_>,
    input: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let index = context.invocation().index_1d();
    let _ = output.store(
        index.into_disjoint(),
        input.load(index.get()).unwrap_or(0.0),
    );
}

fn require_vecadd_arguments(_: vecadd_gpu::Arguments<'_>) {}

fn main() {
    let input = [1.0_f32; 4];
    let mut output = [0.0_f32; 4];
    let copy_arguments = copy_gpu::Arguments::new(
        fe2o3_host::__generated::GeneratedKfdReadSlice::new(&input),
        fe2o3_host::__generated::GeneratedKfdWriteSlice::new(&mut output),
    );
    require_vecadd_arguments(copy_arguments);
}
