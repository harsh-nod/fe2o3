//! Attributed source profiles with compiler-derived registration identity.

#![allow(missing_docs)] // Generated typed-kernel modules do not carry rustdoc in V1.

use fe2o3_device::{
    DisjointSlice, DynamicLds, Gfx942Collectives, GridExclusive, Invocation3D, Workgroup,
    WorkgroupCollectiveScratch, WorkgroupLdsScope, kernel, thread,
};

/// Exact workgroup dimensions for both synchronization profiles.
pub const LDS_REDUCTION_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// The source is type-checked, but its exact LDS compiler profile is not registered.
pub const LDS_REDUCTION_COMPILER_PROFILE_REGISTERED_V1: bool = false;
/// The source ABI is typed, but its atomic compiler profile is not registered.
pub const SCOPED_ATOMIC_COMPILER_PROFILE_REGISTERED_V1: bool = false;

/// Reduces one exact 64-element `i32` row through LDS and writes from lane zero.
///
/// Admitted inputs have a mathematical sum representable by `i32`, making the
/// device collective's wrapping additions equal to the exact host oracle. The
/// public collective implementation performs one unique LDS publish per lane,
/// uniform publish/read barriers, and a final barrier before scratch reuse.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn lds_publish_read_reduce_i32_v1(
    values: &[i32],
    mut output: DisjointSlice<i32, GridExclusive>,
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
        || lane >= 64
    {
        fe2o3_device::trap();
        return;
    }

    let invocation = Invocation3D::current();
    let Some(group) = Workgroup::from_invocation_snapshot(&invocation) else {
        fe2o3_device::trap();
        return;
    };

    let mut lds_scope = WorkgroupLdsScope::current();
    let lds = DynamicLds::<i32>::exact_current::<64>(&mut lds_scope);
    let Ok(mut scratch) = WorkgroupCollectiveScratch::from_dynamic_lds(&group, lds) else {
        fe2o3_device::trap();
        return;
    };
    let context = Gfx942Collectives::current();
    let value = values[lane as usize];
    let sum = group.reduce_sum(&context, &mut scratch, value);

    if lane == 0 {
        let Some(leader) = thread::grid_leader() else {
            fe2o3_device::trap();
            return;
        };
        if let Some(slot) = output.get_mut_exclusive(&leader, 0) {
            *slot = sum;
        } else {
            fe2o3_device::trap();
        }
    }
}
