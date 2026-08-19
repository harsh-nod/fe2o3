use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn divergent_barrier(context: &mut KernelContext<'_>) {
    context.stage(context.lane, context.phase as u32, context.lane as u16);
    if context.lane.is_multiple_of(2) {
        context.publish_barrier();
    }
}
