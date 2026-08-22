use fe2o3_device::{
    DynamicLds, Workgroup, WorkgroupCollectiveScratch, WorkgroupCollectiveScratchError,
    WorkgroupLdsScope,
};

fn bind<'group>(
    scope: &'group mut WorkgroupLdsScope<'group>,
    group: &'group Workgroup<'group>,
) -> Result<WorkgroupCollectiveScratch<'group, i32>, WorkgroupCollectiveScratchError> {
    let lds = DynamicLds::<i32>::exact_current::<64>(scope);
    WorkgroupCollectiveScratch::from_dynamic_lds(group, lds)
}

fn main() {
    let _ = bind;
}
