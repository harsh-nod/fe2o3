use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn unguarded_a_tail_load(context: &mut KernelContext<'_>) {
    let value = context.load_a(context.row(), context.depth());
    context.stage(context.lane, context.phase as u32, value);
}
