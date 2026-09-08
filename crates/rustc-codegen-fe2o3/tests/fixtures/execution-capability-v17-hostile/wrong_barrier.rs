use fe2o3_device::{
    GlobalMemory, InitialEpoch, Relaxed, WorkgroupCapability, WorkgroupScope,
};

fn wrong_order<'workgroup, Brand>(workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>) {
    let _ = workgroup.barrier::<WorkgroupScope, Relaxed, GlobalMemory>();
}

