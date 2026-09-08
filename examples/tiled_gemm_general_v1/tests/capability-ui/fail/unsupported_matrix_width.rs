use fe2o3_device::{InitialEpoch, SubgroupWidth32, WorkgroupCapability};

fn unsupported_width<Brand>(workgroup: &WorkgroupCapability<'_, Brand, InitialEpoch>) {
    let subgroup = workgroup.subgroup::<SubgroupWidth32>();
    subgroup.with_matrix(workgroup.epoch(), |_matrix, _lane| {});
}
