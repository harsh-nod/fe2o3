use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn incorrect_k_tail_zero_fill(context: &mut KernelContext<'_>) {
    let depth = context.depth();
    let value = if depth < context.k {
        context.load_a(context.row(), depth)
    } else {
        0x3f80
    };
    context.stage(context.lane, context.phase as u32, value);
}
