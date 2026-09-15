use gpu_device::{AtomicReadWrite, Global, KernelContext, SystemScope, kernel};
use gpu_host::__generated::GeneratedHostWriteSliceV1;

#[kernel(
    typed,
    namespace = "8c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn atomic(context: KernelContext<'_>, counters: Global<'_, u32, AtomicReadWrite<SystemScope>>) {
    let _ = (context, counters);
}

fn arguments(target: &mut [u32]) -> atomic_gpu::Arguments<'_> {
    atomic_gpu::Arguments::new(GeneratedHostWriteSliceV1::new(target))
}

fn main() {
    let _ = arguments;
}
