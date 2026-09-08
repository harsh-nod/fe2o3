use fe2o3_device::{InitialEpoch, SubgroupWidth32, WorkgroupCapability};

fn reject<Brand>(group: &WorkgroupCapability<'_, Brand, InitialEpoch>) {
    let subgroup = group.subgroup::<SubgroupWidth32>();
    subgroup.with_matrix(group.epoch(), |_matrix, _lane| {});
}

fn main() {}
