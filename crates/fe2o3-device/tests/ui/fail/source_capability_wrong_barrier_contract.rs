use fe2o3_device::{
    AcquireRelease, GlobalMemory, InitialEpoch, Relaxed, SubgroupScope, WorkgroupCapability,
    WorkgroupMemory, WorkgroupScope,
};

fn wrong_scope<Brand>(group: WorkgroupCapability<'_, Brand, InitialEpoch>) {
    let _ = group.barrier::<SubgroupScope, AcquireRelease, WorkgroupMemory>();
}

fn wrong_order<Brand>(group: WorkgroupCapability<'_, Brand, InitialEpoch>) {
    let _ = group.barrier::<WorkgroupScope, Relaxed, GlobalMemory>();
}

fn main() {}
