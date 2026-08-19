use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn unguarded_c_tail_store(context: &mut KernelContext<'_>) {
    context.store_c(context.row(), context.column(), 1.0);
}
