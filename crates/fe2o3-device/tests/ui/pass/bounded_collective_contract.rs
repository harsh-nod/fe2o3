use fe2o3_device::{
    DynamicLds, Gfx942Collectives, Gfx942StaticLdsU32x256, SubgroupTile, Workgroup,
    WorkgroupCollectiveScratch, WorkgroupCollectives,
};

fn exact_wave_lds_contract(
    context: &Gfx942Collectives,
    scratch: &mut Gfx942StaticLdsU32x256,
) {
    let _: u32 = context.wave64_reduce_sum_active_u32(1, 7);
    let _: u32 = context.workgroup256_reduce_sum_active_u32(scratch, 0, 7);
}

fn wave_contract(context: &Gfx942Collectives, tile: &SubgroupTile<'_, 64>) {
    let _: u32 = tile.reduce_sum(context, 1_u32);
    let _: i32 = tile.reduce_sum(context, -1_i32);
    let _: f32 = tile.reduce_sum(context, 1.0_f32);
    let _: u32 = tile.inclusive_scan_sum(context, 1_u32);
    let _: u32 = tile.exclusive_scan_sum(context, 1_u32);
}

fn workgroup_u32_contract<'group>(
    context: &Gfx942Collectives,
    group: &'group Workgroup<'group>,
    scratch: &mut WorkgroupCollectiveScratch<'group, u32>,
) {
    let _: u32 = group.reduce_sum(context, scratch, 1_u32);
    let _: u32 = group.inclusive_scan_sum(context, scratch, 1_u32);
    let _: u32 = group.exclusive_scan_sum(context, scratch, 1_u32);
}

fn workgroup_i32_contract<'group>(
    context: &Gfx942Collectives,
    group: &'group Workgroup<'group>,
    scratch: &mut WorkgroupCollectiveScratch<'group, i32>,
) {
    let _: i32 = group.reduce_sum(context, scratch, -1_i32);
}

fn workgroup_f32_contract<'group>(
    context: &Gfx942Collectives,
    group: &'group Workgroup<'group>,
    scratch: &mut WorkgroupCollectiveScratch<'group, f32>,
) {
    let _: f32 = group.reduce_sum(context, scratch, 1.0_f32);
}

fn portable_workgroup_contract<'group>(
    context: &WorkgroupCollectives,
    u32_scratch: DynamicLds<'group, u32>,
    i32_scratch: DynamicLds<'group, i32>,
    f32_scratch: DynamicLds<'group, f32>,
) {
    let _: u32 = context.reduce_sum_portable(u32_scratch, 1_u32);
    let _: i32 = context.reduce_sum_portable(i32_scratch, -1_i32);
    let _: f32 = context.reduce_sum_portable(f32_scratch, 1.0_f32);
}

fn portable_workgroup_inclusive_scan_contract<'group>(
    context: &WorkgroupCollectives,
    u32_scratch: DynamicLds<'group, u32>,
    i32_scratch: DynamicLds<'group, i32>,
    f32_scratch: DynamicLds<'group, f32>,
) {
    let _: u32 = context.inclusive_scan_sum(u32_scratch, 1_u32);
    let _: i32 = context.inclusive_scan_sum(i32_scratch, -1_i32);
    let _: f32 = context.inclusive_scan_sum(f32_scratch, 1.0_f32);
}

fn portable_workgroup_exclusive_scan_contract<'group>(
    context: &WorkgroupCollectives,
    u32_scratch: DynamicLds<'group, u32>,
    i32_scratch: DynamicLds<'group, i32>,
    f32_scratch: DynamicLds<'group, f32>,
) {
    let _: u32 = context.exclusive_scan_sum(u32_scratch, 1_u32);
    let _: i32 = context.exclusive_scan_sum(i32_scratch, -1_i32);
    let _: f32 = context.exclusive_scan_sum(f32_scratch, 1.0_f32);
}

fn main() {
    let _ = wave_contract;
    let _ = exact_wave_lds_contract;
    let _ = workgroup_u32_contract;
    let _ = workgroup_i32_contract;
    let _ = workgroup_f32_contract;
    let _ = portable_workgroup_contract;
    let _ = portable_workgroup_inclusive_scan_contract;
    let _ = portable_workgroup_exclusive_scan_contract;
}
