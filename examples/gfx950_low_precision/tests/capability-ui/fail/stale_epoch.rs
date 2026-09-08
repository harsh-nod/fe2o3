#![forbid(unsafe_code)]

use fe2o3_device::{InitialEpoch, NextEpoch, Subgroup, SubgroupWidth64, WorkgroupCapability};

fn stale_epoch<'workgroup, Brand>(
    workgroup: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    subgroup: &Subgroup<'workgroup, SubgroupWidth64, Brand, NextEpoch<InitialEpoch>>,
) {
    let _ = subgroup.reduce_sum(workgroup.epoch(), 1.0_f32);
}
