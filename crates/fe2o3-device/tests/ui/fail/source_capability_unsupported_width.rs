use fe2o3_device::{InitialEpoch, WorkgroupCapability};

enum UnsupportedWidth {}

fn reject<Brand>(group: &WorkgroupCapability<'_, Brand, InitialEpoch>) {
    let _ = group.subgroup::<UnsupportedWidth>();
}

fn main() {}
