use fe2o3_device::{
    Global, GlobalAddressSpace, InitialEpoch, ReadOnly, SystemScope, WorkgroupBrand,
    WorkgroupCapability,
};

fn wrong_role<'kernel, 'workgroup, Brand>(
    group: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    view: &Global<'kernel, u32, ReadOnly, Brand>,
) {
    let _: Option<
        fe2o3_device::ScopedAtomic<
            '_,
            u32,
            GlobalAddressSpace,
            SystemScope,
            WorkgroupBrand<'workgroup, Brand>,
            InitialEpoch,
        >,
    > = group.global_atomic(view, 0);
}

fn main() {}
