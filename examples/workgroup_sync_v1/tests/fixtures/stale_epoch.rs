use fe2o3_device::{
    AcquireRelease, InitialEpoch, SubgroupWidth64, WorkgroupCapability, WorkgroupMemory,
    WorkgroupScope,
};

fn stale<'workgroup, Brand>(workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>) {
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let workgroup = workgroup.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>();
    let _ = subgroup.reduce_sum(workgroup.epoch(), 1_u32);
}
