use fe2o3_device::{
    Gfx942Collectives, Gfx942StaticLdsU32x256, SubgroupTile, Workgroup,
    WorkgroupCollectiveScratch,
};

unsafe fn exact_wave_lds_contract(
    context: &Gfx942Collectives,
    scratch: &mut Gfx942StaticLdsU32x256,
) {
    let _: u32 = unsafe { context.wave64_reduce_sum_active_u32(1, 7) };
    let _: u32 =
        unsafe { context.workgroup256_reduce_sum_active_u32(scratch, 0, 7) };
}

unsafe fn wave_contract(context: &Gfx942Collectives, tile: &SubgroupTile<'_, 64>) {
    let _: u32 = unsafe { tile.reduce_sum(context, 1_u32) };
    let _: i32 = unsafe { tile.reduce_sum(context, -1_i32) };
    let _: f32 = unsafe { tile.reduce_sum(context, 1.0_f32) };
    let _: u32 = unsafe { tile.inclusive_scan_sum(context, 1_u32) };
    let _: u32 = unsafe { tile.exclusive_scan_sum(context, 1_u32) };
}

unsafe fn workgroup_u32_contract<'group>(
    context: &Gfx942Collectives,
    group: &'group Workgroup<'group>,
    scratch: &mut WorkgroupCollectiveScratch<'group, u32>,
) {
    let _: u32 = unsafe { group.reduce_sum(context, scratch, 1_u32) };
    let _: u32 = unsafe { group.inclusive_scan_sum(context, scratch, 1_u32) };
    let _: u32 = unsafe { group.exclusive_scan_sum(context, scratch, 1_u32) };
}

unsafe fn workgroup_i32_contract<'group>(
    context: &Gfx942Collectives,
    group: &'group Workgroup<'group>,
    scratch: &mut WorkgroupCollectiveScratch<'group, i32>,
) {
    let _: i32 = unsafe { group.reduce_sum(context, scratch, -1_i32) };
}

unsafe fn workgroup_f32_contract<'group>(
    context: &Gfx942Collectives,
    group: &'group Workgroup<'group>,
    scratch: &mut WorkgroupCollectiveScratch<'group, f32>,
) {
    let _: f32 = unsafe { group.reduce_sum(context, scratch, 1.0_f32) };
}

fn main() {
    let _ = wave_contract;
    let _ = exact_wave_lds_contract;
    let _ = workgroup_u32_contract;
    let _ = workgroup_i32_contract;
    let _ = workgroup_f32_contract;
}
