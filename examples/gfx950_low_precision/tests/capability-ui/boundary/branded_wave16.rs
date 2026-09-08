#![forbid(unsafe_code)]

use fe2o3_device::{InitialEpoch, SubgroupWidth64, WorkgroupCapability};

fn branded_wave16<'workgroup, Brand>(
    workgroup: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    value: f32,
) {
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
    let maximum = wave16.reduce_max_f32(value);
    let sum = wave16.reduce_sum_f32(value);
    let _ = wave16.broadcast_f32(maximum + sum, 0);
}
