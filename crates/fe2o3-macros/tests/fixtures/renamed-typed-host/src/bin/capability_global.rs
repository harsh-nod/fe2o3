use gpu_device::capability_memory::{
    AtomicReadWrite, DisjointWrite, ExclusiveReadWrite, Global, ReadOnly,
};
use gpu_device::{Blocked, Index1D, KernelContext, SystemScope, WriteOnlyDisjointSlice, kernel};

type OutputMapping = Blocked<Index1D, 64, 2>;

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn typed_global(
    context: KernelContext<'_>,
    input: Global<'_, u32, ReadOnly>,
    mut scratch: Global<'_, u32, ExclusiveReadWrite>,
    output: Global<'_, u32, DisjointWrite<Blocked<Index1D, 64, 2>>>,
    counters: Global<'_, u32, AtomicReadWrite<SystemScope>>,
) {
    let _ = context.invocation();
    let _ = input.load(0);
    let _ = scratch.load(0);
    let _ = scratch.store(0, 1);
    let _ = (output.is_empty(), counters.is_empty());
}

fn main() {
    type PhysicalEntry = fn(&[u32], &mut [u32], WriteOnlyDisjointSlice<u32, OutputMapping>, &[u32]);
    let _: PhysicalEntry = <typed_global_gpu::Marker as gpu_device::KernelMarkerV1>::FUNCTION;
    assert_eq!(
        <typed_global_gpu::Marker as gpu_device::KernelMarkerV1>::REGISTRATION.2,
        4,
    );
}
