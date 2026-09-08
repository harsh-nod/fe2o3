use fe2o3_device::prelude::*;

enum Brand {}

fn stale_epoch<'memory, 'workgroup>(
    memory: &WorkgroupMemoryView<
        'memory,
        'workgroup,
        u32,
        ReadOnly,
        Brand,
        InitialEpoch,
    >,
    workgroup: &WorkgroupCapability<'workgroup, Brand, NextEpoch<InitialEpoch>>,
) {
    let _ = memory.load(workgroup, 0);
}

fn main() {}
