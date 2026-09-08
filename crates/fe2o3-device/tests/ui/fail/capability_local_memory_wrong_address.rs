use fe2o3_device::prelude::*;

enum Brand {}

fn private_only(_: &PrivateMemoryView<'_, u32, ReadOnly, Brand>) {}

fn wrong_address<'memory, 'workgroup, Epoch: SynchronizationEpoch>(
    workgroup: &WorkgroupMemoryView<'memory, 'workgroup, u32, ReadOnly, Brand, Epoch>,
) {
    private_only(workgroup);
}

fn main() {}
