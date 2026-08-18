use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn missing_publish_barrier(context: &mut KernelContext<'_>) {
    context.stage(context.lane, context.phase as u32, context.lane as u16);
    let value = context.read_stage((context.lane + 1) % 64, context.phase as u32);
    context.store_c(context.row(), context.column(), value as f32);
}
