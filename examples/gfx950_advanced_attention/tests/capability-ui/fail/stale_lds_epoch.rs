use fe2o3_device::{
    AcquireRelease, InitialEpoch, WorkgroupCapability, WorkgroupMemory, WorkgroupScope,
};

fn stale_epoch<'workgroup, Brand>(workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>) {
    let initialized = workgroup
        .allocate_lds::<f32, 64>()
        .initialize_by_invocation(&workgroup, 1.0);
    let (next, published) = workgroup.publish_lds(initialized);
    let later = next.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>();
    let _ = published.read(&later, 0);
}
