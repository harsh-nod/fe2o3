use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn overlapping_workgroup_c_tile(context: &mut KernelContext<'_>) {
    let row = context.group_y * 16 + context.lane / 16;
    let column = context.lane % 16;
    context.store_c(row, column, context.group_x as f32);
}
