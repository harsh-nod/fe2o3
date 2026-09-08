use fe2o3_device::{InitialEpoch, NextEpoch, ReadOnly, WorkgroupMemoryView};

fn substitute<'memory, 'workgroup, KernelBrand>(
    view: WorkgroupMemoryView<'memory, 'workgroup, u32, ReadOnly, KernelBrand, InitialEpoch>,
) -> WorkgroupMemoryView<
    'memory,
    'workgroup,
    u32,
    ReadOnly,
    KernelBrand,
    NextEpoch<InitialEpoch>,
> {
    view
}

