//! Ordinary attributed Rust source for masked Wave64 collectives.

#![allow(missing_docs)]

use fe2o3_device::{
    DisjointWrite, Global, Index1D, KernelContext, ReadOnly, SubgroupWidth64, kernel,
};

/// Exact launch dimensions required by the Phase A source contract.
pub const WAVE64_COLLECTIVES_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

/// Computes one width-64 subgroup reduction and workgroup scans.
///
/// The mask controls logical participation only. Every physical lane reaches
/// the subgroup reduction and both workgroup collectives in the same order.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        static_shared_memory_bytes = 256
    )
)]
pub fn wave64_collectives_v1(
    mut context: KernelContext<'_>,
    input: Global<'_, f32, ReadOnly>,
    active_mask: u64,
    mut reduction_output: Global<'_, f32, DisjointWrite<Index1D>>,
    mut inclusive_output: Global<'_, f32, DisjointWrite<Index1D>>,
    mut exclusive_output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let invocation = context.invocation();
    let lane = invocation.index_1d().get();
    if lane >= 64
        || input.len() != 64
        || reduction_output.len() != 64
        || inclusive_output.len() != 64
        || exclusive_output.len() != 64
    {
        fe2o3_device::trap();
    }
    let Some(input_value) = input.load(lane) else {
        fe2o3_device::trap();
    };
    let active = active_mask & (1_u64 << lane) != 0;
    let contribution = if active { input_value } else { 0.0_f32 };

    let (reduction, inclusive, exclusive) = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let reduction = subgroup.reduce_sum(workgroup.epoch(), contribution);
        let scratch = workgroup.allocate_lds::<f32, 64>();
        let (workgroup, scratch, inclusive) = workgroup.inclusive_scan_sum(scratch, contribution);
        let (_workgroup, _scratch, exclusive) = workgroup.exclusive_scan_sum(scratch, contribution);
        (reduction, inclusive, exclusive)
    });

    let published_reduction = if active { reduction } else { 0.0 };
    let published_inclusive = if active { inclusive } else { 0.0 };
    let published_exclusive = if active { exclusive } else { 0.0 };
    if !reduction_output.store(
        context.invocation().index_1d().into_disjoint(),
        published_reduction,
    ) || !inclusive_output.store(
        context.invocation().index_1d().into_disjoint(),
        published_inclusive,
    ) || !exclusive_output.store(
        context.invocation().index_1d().into_disjoint(),
        published_exclusive,
    ) {
        fe2o3_device::trap();
    }
}
