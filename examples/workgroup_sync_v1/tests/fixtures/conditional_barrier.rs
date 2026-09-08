use fe2o3_device::{
    AcquireRelease, InitialEpoch, WorkgroupCapability, WorkgroupMemory, WorkgroupScope,
};

fn conditional<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    take_barrier: bool,
) {
    let _workgroup = if take_barrier {
        workgroup.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>()
    } else {
        workgroup
    };
}
