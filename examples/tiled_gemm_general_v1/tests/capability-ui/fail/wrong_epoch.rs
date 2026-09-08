use fe2o3_device::{
    AcquireRelease, InitialEpoch, SubgroupWidth64, WorkgroupCapability, WorkgroupMemory,
    WorkgroupScope,
};

fn wrong_epoch<'workgroup, Brand>(workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>) {
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let workgroup = workgroup.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>();
    subgroup.with_matrix(workgroup.epoch(), |_matrix, _lane| {});
}
