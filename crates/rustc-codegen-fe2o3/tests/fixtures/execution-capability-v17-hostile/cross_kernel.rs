use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, PrivateMemoryView, ReadOnly, RegisteredLaunch,
};

fn substitute<'memory, Left, Right>(
    view: PrivateMemoryView<
        'memory,
        u32,
        ReadOnly,
        KernelCapabilityBrand<'memory, Left, CurrentTarget, RegisteredLaunch>,
    >,
) -> PrivateMemoryView<
    'memory,
    u32,
    ReadOnly,
    KernelCapabilityBrand<'memory, Right, CurrentTarget, RegisteredLaunch>,
> {
    view
}

