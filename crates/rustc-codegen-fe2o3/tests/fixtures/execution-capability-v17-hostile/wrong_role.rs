use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, PrivateMemoryView, ReadOnly, RegisteredLaunch,
};

fn write_through_read_only<'memory, Kernel>(
    view: &mut PrivateMemoryView<
        'memory,
        u32,
        ReadOnly,
        KernelCapabilityBrand<'memory, Kernel, CurrentTarget, RegisteredLaunch>,
    >,
) {
    let _ = view.store(0, 1);
}

