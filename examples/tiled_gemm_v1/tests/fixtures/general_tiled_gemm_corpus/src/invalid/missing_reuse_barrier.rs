use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn missing_reuse_barrier(context: &mut KernelContext<'_>) {
    context.stage(context.lane, 0, context.lane as u16);
    context.publish_barrier();
    let _prior = context.read_stage(context.lane, 0);
    context.stage(context.lane, 1, 0);
}
