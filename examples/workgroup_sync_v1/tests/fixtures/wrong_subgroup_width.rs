use fe2o3_device::{
    AcquireRelease, GlobalAndWorkgroupMemory, InitialEpoch, SubgroupScope, SubgroupWidth32,
    SubgroupWidth64, WorkgroupCapability,
};

fn wrong_width<'workgroup, Brand>(workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>) {
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let _ = workgroup.subgroup_barrier::<
        SubgroupWidth32,
        SubgroupScope,
        AcquireRelease,
        GlobalAndWorkgroupMemory,
    >(subgroup);
}
