//! Ordinary-Rust `u32` acceptance entry for target-neutral workgroup scan.

use fe2o3_device::{
    DisjointSlice, DynamicLds, StridedReadView2D, WorkgroupCollectives, WorkgroupLdsScope, kernel,
    thread,
};

macro_rules! inclusive_scan_u32_body {
    ($values:ident, $output:ident, $extent:literal) => {{
        if $output.len() != $extent
            || thread::block_dim_x() != $extent
            || thread::block_dim_y() != 1
            || thread::block_dim_z() != 1
            || thread::thread_idx_y() != 0
            || thread::thread_idx_z() != 0
            || thread::block_idx_x() != 0
            || thread::block_idx_y() != 0
            || thread::block_idx_z() != 0
        {
            fe2o3_device::trap();
        }
        let Ok(input) =
            StridedReadView2D::from_shared_slice($values, 0, 1, $extent, $extent)
        else {
            fe2o3_device::trap();
        };
        let mut scope = WorkgroupLdsScope::current();
        let scratch = DynamicLds::<u32>::exact_current::<$extent>(&mut scope);
        let collective = WorkgroupCollectives::current();
        let prefix = collective.inclusive_scan_sum(
            scratch,
            input.load_or(0, thread::index_1d().get(), 0),
        );
        let Some(slot) = $output.get_mut(thread::index_1d()) else {
            fe2o3_device::trap();
        };
        *slot = prefix;
    }};
}

#[cfg(feature = "lds-scan-u32-kernel")]
/// Computes the inclusive prefix sum of one exact 3-element `u32` row.
#[kernel(
    typed,
    launch(required = [3, 1, 1], max = [3, 1, 1], static_shared_memory_bytes = 12)
)]
pub fn lds_inclusive_scan_u32_v1(values: &[u32], mut output: DisjointSlice<u32>) {
    inclusive_scan_u32_body!(values, output, 3);
}

#[cfg(feature = "lds-scan-u32-65-kernel")]
/// Computes the inclusive prefix sum of one exact 65-element `u32` row.
#[kernel(
    typed,
    launch(required = [65, 1, 1], max = [65, 1, 1], static_shared_memory_bytes = 260)
)]
pub fn lds_inclusive_scan_u32_65_v1(values: &[u32], mut output: DisjointSlice<u32>) {
    inclusive_scan_u32_body!(values, output, 65);
}

#[cfg(feature = "lds-scan-u32-255-kernel")]
/// Computes the inclusive prefix sum of one exact 255-element `u32` row.
#[kernel(
    typed,
    launch(
        required = [255, 1, 1],
        max = [255, 1, 1],
        static_shared_memory_bytes = 1020
    )
)]
pub fn lds_inclusive_scan_u32_255_v1(values: &[u32], mut output: DisjointSlice<u32>) {
    inclusive_scan_u32_body!(values, output, 255);
}
