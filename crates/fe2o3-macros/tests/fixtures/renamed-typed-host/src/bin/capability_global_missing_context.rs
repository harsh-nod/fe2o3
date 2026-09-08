use gpu_device::capability_memory::{Global, ReadOnly};
use gpu_device::kernel;

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn missing_context(input: Global<'_, u32, ReadOnly>) {
    let _ = input;
}

fn main() {}
