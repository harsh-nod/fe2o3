#![no_std]

use fe2o3_device::{
    InitialEpoch, KernelContext, SubgroupWidth64, WorkgroupCapability, WorkgroupEpoch, kernel,
};

fn epoch_ref<'a, 'w, Brand>(
    workgroup: &'a WorkgroupCapability<'w, Brand>,
) -> &'a WorkgroupEpoch<'w, Brand, InitialEpoch> {
    workgroup.epoch()
}

fn nested_epoch_ref<'a, 'w, Brand>(
    workgroup: &'a WorkgroupCapability<'w, Brand>,
) -> &'a WorkgroupEpoch<'w, Brand, InitialEpoch> {
    epoch_ref(workgroup)
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn workgroup_borrow_import(mut context: KernelContext<'_>, value: f32) {
    let _value = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let first = subgroup.gfx950_wave16(epoch_ref(&workgroup));
        let second = subgroup.gfx950_wave16(nested_epoch_ref(&workgroup));
        second.broadcast_f32(first.reduce_sum_f32(value), 3)
    });
}
