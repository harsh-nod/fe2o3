use gpu_device::capability_memory::Global;
use gpu_device::{KernelContext, kernel};

enum ForgedRole {}

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn wrong_role(context: KernelContext<'_>, input: Global<'_, u32, ForgedRole>) {
    let _ = (context, input);
}

fn main() {}
