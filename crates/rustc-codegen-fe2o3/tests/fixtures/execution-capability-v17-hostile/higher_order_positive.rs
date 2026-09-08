#![no_std]

use fe2o3_device::{KernelContext, SubgroupWidth64, kernel};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn authenticated_higher_order_capabilities(mut context: KernelContext<'_>, _seed: u32) {
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |_matrix, _lane| ());
    });
}
