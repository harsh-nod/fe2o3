use fe2o3_device::capability_memory::{Global, ReadOnly};
use fe2o3_device::{KernelContext, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn wrong_write_role(context: KernelContext<'_>, mut output: Global<'_, f32, ReadOnly>) {
    let index = context.invocation().index_1d().into_disjoint();
    let _ = output.store(index, 1.0);
}

fn main() {}
