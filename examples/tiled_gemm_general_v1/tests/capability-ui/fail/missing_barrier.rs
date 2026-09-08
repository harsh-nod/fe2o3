use fe2o3_device::{InitialEpoch, WorkgroupCapability};

fn missing_barrier<'workgroup, Brand>(
    workgroup: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
) {
    let lds = workgroup.allocate_lds::<u16, 64>();
    let _ = lds.read(workgroup, 0);
}
