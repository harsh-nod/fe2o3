use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn staged_read_before_wait(context: &mut KernelContext<'_>) {
    context.begin_async_stage(context.lane, context.phase as u32, context.lane as u16);
    let _premature = context.read_stage(context.lane, context.phase as u32);
}
