//! Attributed `f32` reduction using the shared capability source shape.

use fe2o3_device::{DisjointWrite, Global, Index1D, KernelContext, ReadOnly, kernel};

use crate::capability_collectives::execute_workgroup_collectives_v1;

/// Reduces one exact 64-element `f32` row.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        static_shared_memory_bytes = 256
    )
)]
pub fn lds_publish_read_reduce_f32_v1(
    mut context: KernelContext<'_>,
    values: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let invocation = context.invocation();
    let lane = invocation.index_1d().get();
    if values.len() != 64 || output.len() != 1 {
        fe2o3_device::trap();
    }
    let Some(value) = values.load(lane) else {
        fe2o3_device::trap();
    };
    let result = context.with_workgroup(|workgroup| {
        execute_workgroup_collectives_v1::<f32, 64, _>(workgroup, value)
    });
    if lane == 0
        && !output.store(
            context.invocation().index_1d().into_disjoint(),
            result.reduction,
        )
    {
        fe2o3_device::trap();
    }
}
