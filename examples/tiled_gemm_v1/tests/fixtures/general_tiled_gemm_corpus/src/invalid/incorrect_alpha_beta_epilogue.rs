use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn incorrect_alpha_beta_epilogue(context: &mut KernelContext<'_>) {
    let product = context.lane as f32;
    let initial = context.load_c(context.row(), context.column());
    let wrong = context.alpha * product + initial;
    context.store_c(context.row(), context.column(), wrong);
}
