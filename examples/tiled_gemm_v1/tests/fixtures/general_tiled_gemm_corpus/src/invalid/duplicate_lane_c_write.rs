use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn duplicate_lane_c_write(context: &mut KernelContext<'_>) {
    let duplicate_index = context.group_y * context.ldc + context.group_x;
    context.store_c_index(duplicate_index, context.lane as f32);
}
