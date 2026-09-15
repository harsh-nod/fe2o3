use gpu_device::{AtomicReadWrite, Global, KernelContext, SystemScope, kernel};
use gpu_host::__generated::GeneratedHostReadWriteSliceV1;

#[kernel(
    typed,
    namespace = "8c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn atomic_pair(
    context: KernelContext<'_>,
    left: Global<'_, u32, AtomicReadWrite<SystemScope>>,
    right: Global<'_, u32, AtomicReadWrite<SystemScope>>,
) {
    let _ = (context, left, right);
}

fn arguments(target: &mut [u32]) {
    let left = GeneratedHostReadWriteSliceV1::new(target);
    let right = GeneratedHostReadWriteSliceV1::new(target);
    let _arguments: atomic_pair_gpu::Arguments<'_> = atomic_pair_gpu::Arguments::new(left, right);
}

fn main() {
    let _ = arguments;
}
