use fe2o3_device::{InitialEpoch, WorkgroupCapability};

fn missing<'workgroup, Brand>(workgroup: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>) {
    let memory = workgroup.allocate_memory::<u32, 64>();
    let _ = memory.load(workgroup, 0);
}
