use fe2o3_device::{
    AcquireRelease, InitialEpoch, WorkgroupCapability, WorkgroupMemory, WorkgroupScope,
};

fn reordered_barrier<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
) {
    let initialized = workgroup
        .allocate_lds::<u16, 64>()
        .initialize_by_invocation(&workgroup, 0);
    let workgroup = workgroup.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>();
    let _ = workgroup.publish_lds(initialized);
}
