//! Attributed `i32` reduction using compiler-issued execution capabilities.

#![allow(missing_docs)]

use fe2o3_device::{DisjointWrite, Global, Index1D, KernelContext, ReadOnly, kernel};

use crate::capability_collectives::execute_workgroup_collectives_v1;

/// Exact workgroup dimensions for both synchronization profiles.
pub const LDS_REDUCTION_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

/// Reduces one exact 64-element `i32` row and publishes from lane zero.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        static_shared_memory_bytes = 256
    )
)]
pub fn lds_publish_read_reduce_i32_v1(
    mut context: KernelContext<'_>,
    values: Global<'_, i32, ReadOnly>,
    mut output: Global<'_, i32, DisjointWrite<Index1D>>,
) {
    let invocation = context.invocation();
    let lane = invocation.index_1d().get();
    if values.len() != 64
        || output.len() != 1
        || invocation.workgroup_size().volume() != 64
        || invocation.grid_size().volume() != Some(1)
    {
        fe2o3_device::trap();
    }
    let Some(value) = values.load(lane) else {
        fe2o3_device::trap();
    };
    let result = context.with_workgroup(|workgroup| {
        execute_workgroup_collectives_v1::<i32, 64, _>(workgroup, value)
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
