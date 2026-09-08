#![allow(deprecated)]

use fe2o3_device::capability_memory::{
    Private, PrivateMemoryView, ReadOnly, Workgroup, WorkgroupMemoryView,
};
use fe2o3_device::{InitialEpoch, KernelCapabilityBrand, RegisteredLaunch};

enum Kernel {}

type Brand<'kernel> = KernelCapabilityBrand<
    'kernel,
    Kernel,
    fe2o3_device::CurrentTarget,
    RegisteredLaunch,
>;

fn private_alias_is_exact<'memory, 'kernel>(
    view: Private<'memory, u32, ReadOnly, Brand<'kernel>>,
) -> PrivateMemoryView<'memory, u32, ReadOnly, Brand<'kernel>> {
    view
}

fn workgroup_alias_is_exact<'workgroup, 'kernel>(
    view: Workgroup<'workgroup, u32, ReadOnly, Brand<'kernel>>,
) -> WorkgroupMemoryView<
    'workgroup,
    'workgroup,
    u32,
    ReadOnly,
    Brand<'kernel>,
    InitialEpoch,
> {
    view
}

fn main() {
    let _ = private_alias_is_exact;
    let _ = workgroup_alias_is_exact;
}
