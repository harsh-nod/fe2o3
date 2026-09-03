//! Ordinary-Rust `f32` acceptance entry for target-neutral workgroup scan.

use fe2o3_device::{
    DisjointSlice, DynamicLds, WorkgroupCollectives, WorkgroupLdsScope, kernel, thread,
};

/// Computes the inclusive prefix sum of one exact 64-element `f32` row.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        static_shared_memory_bytes = 256
    )
)]
pub fn lds_inclusive_scan_f32_v1(values: &[f32], mut output: DisjointSlice<f32>) {
    let lane = thread::thread_idx_x();
    if values.len() != 64
        || output.len() != 64
        || thread::launch_extent_1d() != 64
        || thread::block_dim_x() != 64
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
    let mut scope = WorkgroupLdsScope::current();
    let scratch = DynamicLds::<f32>::exact_current::<64>(&mut scope);
    let collective = WorkgroupCollectives::current();
    let prefix = collective.inclusive_scan_sum(scratch, values[lane as usize]);
    let Some(slot) = output.get_mut(thread::index_1d()) else {
        fe2o3_device::trap();
    };
    *slot = prefix;
}
