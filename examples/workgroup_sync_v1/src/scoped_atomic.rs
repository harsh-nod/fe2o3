//! Ordinary attributed source for a system-scoped atomic addition.

#![allow(missing_docs)]

use fe2o3_device::{
    AtomicReadWrite, Global, GlobalAddressSpace, KernelContext, ReadOnly, Relaxed, SystemScope,
    kernel,
};

/// Adds each eligible lane's value exactly once to one global atomic object.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn scoped_atomic_add_u32_v1(
    mut context: KernelContext<'_>,
    values: Global<'_, u32, ReadOnly>,
    eligible: Global<'_, u32, ReadOnly>,
    target: Global<'_, u32, AtomicReadWrite<SystemScope>>,
) {
    let lane = context.invocation().index_1d().get();
    if values.len() != 64 || eligible.len() != 64 || target.len() != 1 || lane >= 64 {
        fe2o3_device::trap();
    }
    let Some(is_eligible) = eligible.load(lane) else {
        fe2o3_device::trap();
    };
    let Some(value) = values.load(lane) else {
        fe2o3_device::trap();
    };
    context.with_workgroup(|workgroup| {
        if is_eligible != 0 {
            let Some(location) = workgroup.global_atomic(&target, 0) else {
                fe2o3_device::trap();
            };
            let _ = workgroup.atomic_fetch_add::<u32, GlobalAddressSpace, SystemScope, Relaxed>(
                &location, value,
            );
        }
    });
}
