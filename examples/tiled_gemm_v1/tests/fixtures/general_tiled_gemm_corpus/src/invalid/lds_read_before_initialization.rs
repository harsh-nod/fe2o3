use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn lds_read_before_initialization(context: &mut KernelContext<'_>) {
    let value = context.read_stage(context.lane, context.phase as u32);
    context.store_c(context.row(), context.column(), value as f32);
}
