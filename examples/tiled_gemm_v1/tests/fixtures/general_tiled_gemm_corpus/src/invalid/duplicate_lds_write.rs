use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel]
pub fn duplicate_lds_write(context: &mut KernelContext<'_>) {
    let colliding_slot = context.lane % 16;
    context.stage(colliding_slot, context.phase as u32, context.lane as u16);
}
