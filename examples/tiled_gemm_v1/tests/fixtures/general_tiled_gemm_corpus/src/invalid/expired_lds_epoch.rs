use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn expired_lds_epoch(context: &mut KernelContext<'_>) {
    context.stage(context.lane, 0, context.lane as u16);
    context.publish_barrier();
    context.reuse_barrier();
    context.stage(context.lane, 1, 0);
    context.publish_barrier();
    let _expired = context.read_stage(context.lane, 0);
}
