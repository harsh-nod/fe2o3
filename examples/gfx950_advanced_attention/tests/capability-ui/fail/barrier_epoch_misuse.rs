use fe2o3_device::{
    AcquireRelease, InitialEpoch, SubgroupWidth64, WorkgroupCapability, WorkgroupMemory,
    WorkgroupScope,
};

fn reordered_collective<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
) {
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let later = workgroup.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>();
    let _ = subgroup.reduce_sum(later.epoch(), 1.0_f32);
}
