use fe2o3_device::capability_memory::{Global, ReadOnly};
use fe2o3_device::{KernelContext, UnbrandedCapability, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn forged_view(
    context: KernelContext<'_>,
    input: Global<'_, f32, ReadOnly, UnbrandedCapability>,
) {
    let _ = (context.invocation(), input);
}

fn main() {}
