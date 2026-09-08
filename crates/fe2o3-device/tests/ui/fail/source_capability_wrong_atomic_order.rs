use fe2o3_device::{
    GlobalAddressSpace, InitialEpoch, Release, ScopedAtomic, SystemScope, WorkgroupBrand,
    WorkgroupCapability,
};

fn wrong_order<'memory, 'workgroup, Brand>(
    group: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    location: &ScopedAtomic<
        'memory,
        u32,
        GlobalAddressSpace,
        SystemScope,
        WorkgroupBrand<'workgroup, Brand>,
        InitialEpoch,
    >,
) {
    let _ = group.atomic_load::<u32, GlobalAddressSpace, SystemScope, Release>(location);
}

fn main() {}
