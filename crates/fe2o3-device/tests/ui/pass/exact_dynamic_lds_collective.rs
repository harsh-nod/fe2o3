use fe2o3_device::{
    DynamicLds, Workgroup, WorkgroupCollectiveScratch, WorkgroupCollectiveScratchError,
    WorkgroupLdsScope,
};

fn bind<'group>(
    scope: &'group mut WorkgroupLdsScope<'group>,
    group: &'group Workgroup<'group>,
    epoch: u32,
) -> Result<WorkgroupCollectiveScratch<'group, i32>, WorkgroupCollectiveScratchError> {
    // SAFETY: an authenticated exact profile supplies this capability.
    let lds = unsafe { DynamicLds::<i32>::exact_from_compiler::<64>(scope, epoch) };
    WorkgroupCollectiveScratch::from_dynamic_lds(group, lds)
}

fn main() {
    let _ = bind;
}
