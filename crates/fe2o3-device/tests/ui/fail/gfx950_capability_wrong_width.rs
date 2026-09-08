use fe2o3_device::{InitialEpoch, Subgroup, SubgroupWidth32, WorkgroupEpoch};

fn reject<'a>(
    subgroup: &Subgroup<'a, SubgroupWidth32, (), InitialEpoch>,
    epoch: &WorkgroupEpoch<'a, (), InitialEpoch>,
) {
    let _ = subgroup.gfx950_wave16(epoch);
}

fn main() {}
