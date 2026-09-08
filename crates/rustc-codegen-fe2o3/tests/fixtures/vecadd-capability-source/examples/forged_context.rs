use fe2o3_device::capability_memory::{DisjointWrite, Global, ReadOnly};
use fe2o3_device::{Index1D, KernelContext, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn forged_context(
    context: KernelContext<'_>,
    a: Global<'_, f32, ReadOnly>,
    b: Global<'_, f32, ReadOnly>,
    mut c: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let _context = KernelContext::default();
    let index = context.invocation().index_1d();
    let i = index.get();
    let _ = c.store(
        index.into_disjoint(),
        a.load(i).unwrap_or(0.0) + b.load(i).unwrap_or(0.0),
    );
}

fn main() {}
