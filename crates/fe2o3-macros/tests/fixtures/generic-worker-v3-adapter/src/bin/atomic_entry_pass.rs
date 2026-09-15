use gpu_device::{AtomicReadWrite, Global, KernelContext, SystemScope, kernel};
use gpu_host::__generated::GeneratedHostReadWriteSliceV1;

#[kernel(
    typed,
    namespace = "8c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn atomic(context: KernelContext<'_>, counters: Global<'_, u32, AtomicReadWrite<SystemScope>>) {
    let _ = (context, counters);
}

fn arguments(target: &mut [u32]) -> atomic_gpu::Arguments<'_> {
    atomic_gpu::Arguments::new(GeneratedHostReadWriteSliceV1::new(target))
}

// Compatibility check only. This does not justify mutation through plain &[T].
fn role_physical<'a>(
    value: <AtomicReadWrite<SystemScope> as gpu_device::capability_memory::MemoryRole>::Physical<
        'a,
        u32,
    >,
) -> &'a [u32] {
    value
}

fn main() {
    let _: fn(&mut [u32]) = <atomic_gpu::Marker as gpu_device::KernelMarkerV1>::FUNCTION;
    let mut target = [0_u32; 4];
    let _arguments = arguments(&mut target);
    let _ = role_physical;
}
