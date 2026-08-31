//! Ordinary-Rust `f32` acceptance entry for the neutral reduction contract.

use fe2o3_device::{
    DisjointSlice, DynamicLds, GridExclusive, WorkgroupCollectives, WorkgroupLdsScope, kernel,
    thread,
};

/// Reduces one exact 64-element `f32` row through target-neutral LDS lowering.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        static_shared_memory_bytes = 256
    )
)]
pub fn lds_publish_read_reduce_f32_v1(
    values: &[f32],
    mut output: DisjointSlice<f32, GridExclusive>,
) {
    let lane = thread::thread_idx_x();
    let launch_extent = thread::launch_extent_1d();
    if values.len() != 64
        || output.len() != 1
        || launch_extent != 64
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
    let mut lds_scope = WorkgroupLdsScope::current();
    let lds = DynamicLds::<f32>::exact_current::<64>(&mut lds_scope);
    let context = WorkgroupCollectives::current();
    let sum = context.reduce_sum_portable(lds, values[lane as usize]);
    if lane == 0 {
        let Some(leader) = thread::grid_leader() else {
            fe2o3_device::trap();
        };
        if let Some(slot) = output.get_mut_exclusive(&leader, 0) {
            *slot = sum;
        } else {
            fe2o3_device::trap();
        }
    }
}
