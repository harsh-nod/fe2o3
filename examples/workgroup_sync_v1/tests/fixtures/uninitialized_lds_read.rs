use fe2o3_device::{InitialEpoch, WorkgroupCapability};

fn read_before_publish<'workgroup, Brand>(
    workgroup: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
) {
    let lds = workgroup.allocate_lds::<u32, 64>();
    let _ = lds.read(workgroup, 0);
}
