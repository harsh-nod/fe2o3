use fe2o3_device::{
    AcquireRelease, GlobalAndWorkgroupMemory, InitialEpoch, Subgroup, SubgroupScope,
    SubgroupWidth64, WorkgroupCapability,
};

fn wrong_brand<'workgroup, Left, Right>(
    workgroup: WorkgroupCapability<'workgroup, Left, InitialEpoch>,
    subgroup: Subgroup<'workgroup, SubgroupWidth64, Right, InitialEpoch>,
) {
    let _ = workgroup.subgroup_barrier::<
        SubgroupWidth64,
        SubgroupScope,
        AcquireRelease,
        GlobalAndWorkgroupMemory,
    >(subgroup);
}
