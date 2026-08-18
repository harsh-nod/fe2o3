use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn accumulator_reset(context: &mut KernelContext<'_>) {
    let prior_accumulator = context.load_c(context.row(), context.column());
    let phase_contribution = context.lane as f32;
    let accumulator = if context.phase == 0 {
        prior_accumulator + phase_contribution
    } else {
        phase_contribution
    };
    context.store_c(context.row(), context.column(), accumulator);
}
