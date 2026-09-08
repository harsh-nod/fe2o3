use fe2o3_device::{InitialEpoch, ReadOnly, WorkgroupMemoryView};

fn substitute<'memory, 'left, 'right, KernelBrand>(
    view: WorkgroupMemoryView<'memory, 'left, u32, ReadOnly, KernelBrand, InitialEpoch>,
) -> WorkgroupMemoryView<'memory, 'right, u32, ReadOnly, KernelBrand, InitialEpoch> {
    view
}

