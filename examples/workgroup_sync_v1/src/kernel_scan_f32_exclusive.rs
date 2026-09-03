//! Ordinary-Rust `f32` exclusive acceptance entry for target-neutral workgroup scan.

use fe2o3_device::{
    DisjointSlice, DynamicLds, WorkgroupCollectives, WorkgroupLdsScope, kernel, thread,
};

/// Computes the exclusive prefix sum of one exact 65-element `f32` row.
#[kernel(
    typed,
    launch(
        required = [65, 1, 1],
        max = [65, 1, 1],
        static_shared_memory_bytes = 260
    )
)]
pub fn lds_exclusive_scan_f32_v1(values: &[f32], mut output: DisjointSlice<f32>) {
    let lane = thread::thread_idx_x();
    if values.len() != 65
        || output.len() != 65
        || thread::launch_extent_1d() != 65
        || thread::block_dim_x() != 65
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
    let scratch = DynamicLds::<f32>::exact_current::<65>(&mut scope);
    let collective = WorkgroupCollectives::current();
    let prefix = collective.exclusive_scan_sum(scratch, values[lane as usize]);
    let Some(slot) = output.get_mut(thread::index_1d()) else {
        fe2o3_device::trap();
    };
    *slot = prefix;
}
