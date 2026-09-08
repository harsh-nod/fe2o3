use fe2o3_device::prelude::*;

enum Kernel {}

type Brand<'kernel> =
    KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn forge<'memory, 'kernel: 'memory>(
    context: &'memory KernelContext<'kernel, Kernel>,
    pointer: *mut u32,
) -> PrivateMemoryView<'memory, u32, ExclusiveReadWrite, Brand<'kernel>> {
    PrivateMemoryView::from_raw_parts(
        context,
        pointer,
        4,
        PrivateMemoryView::<u32, ExclusiveReadWrite, Brand<'kernel>>::UNSAFE_RAW_OBLIGATION_V1,
    )
}

fn main() {}
