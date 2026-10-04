use fe2o3_device::{Workgroup, WorkgroupCollectiveScratch};

fn reject<'group>(
    group: &'group Workgroup<'group>,
    base: *mut u32,
) -> WorkgroupCollectiveScratch<'group, u32> {
    WorkgroupCollectiveScratch::from_raw_parts(group, base, 64).unwrap()
}

fn main() {}
