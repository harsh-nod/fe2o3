use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn unguarded_b_tail_load(context: &mut KernelContext<'_>) {
    let value = context.load_b(context.depth(), context.column());
    context.stage(256 + context.lane, context.phase as u32, value);
}
