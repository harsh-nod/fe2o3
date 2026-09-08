use fe2o3_device::capability_memory::{DisjointWrite, Global};
use fe2o3_device::{Index1D, KernelContext, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn wrong_read_role(context: KernelContext<'_>, input: Global<'_, f32, DisjointWrite<Index1D>>) {
    let _ = context.invocation();
    let _ = input.load(0);
}

fn main() {}
