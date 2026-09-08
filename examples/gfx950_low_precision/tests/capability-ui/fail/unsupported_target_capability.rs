#![forbid(unsafe_code)]

use fe2o3_device::{InitialEpoch, SubgroupWidth32, WorkgroupCapability};

fn unsupported_matrix_width<'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
) {
    let subgroup = workgroup.subgroup::<SubgroupWidth32>();
    subgroup.with_matrix(workgroup.epoch(), |_matrix, _lane| {});
}
