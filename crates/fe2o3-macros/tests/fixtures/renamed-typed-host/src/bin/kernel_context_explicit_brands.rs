use gpu_device::{CurrentTarget, KernelContext, RegisteredLaunch, UnboundKernel, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn explicit_context_brands(
    context: KernelContext<'_, UnboundKernel, CurrentTarget, RegisteredLaunch>,
    value: u32,
) {
    let _ = (context, value);
}

fn main() {}
