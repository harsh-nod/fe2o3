use gpu_device::{KernelContext, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn named_context_lifetime<'kernel>(context: KernelContext<'kernel>, value: u32) {
    let _ = (context, value);
}

fn main() {}
