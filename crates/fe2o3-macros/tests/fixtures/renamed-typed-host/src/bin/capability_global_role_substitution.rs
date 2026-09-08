use gpu_device::capability_memory::{Global, ReadOnly};
use gpu_device::{KernelContext, kernel};

type ExclusiveReadWrite = ReadOnly;

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn substituted_role(context: KernelContext<'_>, input: Global<'_, u32, ExclusiveReadWrite>) {
    let _ = (context, input);
}

fn main() {}
