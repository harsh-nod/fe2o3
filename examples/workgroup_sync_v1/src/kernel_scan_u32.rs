//! Attributed `u32` inclusive scans using the shared capability source shape.

use fe2o3_device::{DisjointWrite, Global, Index1D, KernelContext, ReadOnly, kernel};

use crate::capability_collectives::execute_workgroup_collectives_v1;

macro_rules! inclusive_scan_u32_body {
    ($context:ident, $values:ident, $output:ident, $extent:literal) => {{
        let lane = $context.invocation().index_1d().get();
        if $values.len() != $extent || $output.len() != $extent || lane >= $extent {
            fe2o3_device::trap();
        }
        let Some(value) = $values.load(lane) else {
            fe2o3_device::trap();
        };
        let result = $context.with_workgroup(|workgroup| {
            execute_workgroup_collectives_v1::<u32, $extent, _>(workgroup, value)
        });
        if !$output.store(
            $context.invocation().index_1d().into_disjoint(),
            result.inclusive,
        ) {
            fe2o3_device::trap();
        }
    }};
}

#[cfg(feature = "lds-scan-u32-kernel")]
/// Computes the inclusive prefix sum of one exact 3-element `u32` row.
#[kernel(typed, launch(required = [3, 1, 1], max = [3, 1, 1], max_grid = [1, 1, 1], static_shared_memory_bytes = 12))]
pub fn lds_inclusive_scan_u32_v1(
    mut context: KernelContext<'_>,
    values: Global<'_, u32, ReadOnly>,
    mut output: Global<'_, u32, DisjointWrite<Index1D>>,
) {
    inclusive_scan_u32_body!(context, values, output, 3);
}

#[cfg(feature = "lds-scan-u32-65-kernel")]
/// Computes the inclusive prefix sum of one exact 65-element `u32` row.
#[kernel(typed, launch(required = [65, 1, 1], max = [65, 1, 1], max_grid = [1, 1, 1], static_shared_memory_bytes = 260))]
pub fn lds_inclusive_scan_u32_65_v1(
    mut context: KernelContext<'_>,
    values: Global<'_, u32, ReadOnly>,
    mut output: Global<'_, u32, DisjointWrite<Index1D>>,
) {
    inclusive_scan_u32_body!(context, values, output, 65);
}

#[cfg(feature = "lds-scan-u32-255-kernel")]
/// Computes the inclusive prefix sum of one exact 255-element `u32` row.
#[kernel(typed, launch(required = [255, 1, 1], max = [255, 1, 1], max_grid = [1, 1, 1], static_shared_memory_bytes = 1020))]
pub fn lds_inclusive_scan_u32_255_v1(
    mut context: KernelContext<'_>,
    values: Global<'_, u32, ReadOnly>,
    mut output: Global<'_, u32, DisjointWrite<Index1D>>,
) {
    inclusive_scan_u32_body!(context, values, output, 255);
}
