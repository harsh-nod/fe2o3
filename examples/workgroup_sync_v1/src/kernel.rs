//! Attributed source profiles and their explicit compiler-registration boundary.

#![allow(missing_docs)] // Generated typed-kernel modules do not carry rustdoc in V1.

use fe2o3_device::{
    DisjointSlice, Gfx942Collectives, GridSize, Invocation3D, Workgroup,
    WorkgroupCollectiveScratch, WorkgroupId, WorkgroupSize, WorkitemId, kernel, thread,
};

/// Exact workgroup dimensions for both synchronization profiles.
pub const LDS_REDUCTION_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// The source is type-checked, but its compiler-supplied LDS base is not registered.
pub const LDS_REDUCTION_COMPILER_PROFILE_REGISTERED_V1: bool = false;
/// The typed ABI cannot yet represent the atomic profile's explicit global pointer.
pub const SCOPED_ATOMIC_COMPILER_PROFILE_REGISTERED_V1: bool = false;
/// Exact quarantined attributed source for the scoped atomic-add profile.
pub const SCOPED_ATOMIC_SOURCE_V1: &str = include_str!("quarantined/scoped_atomic_add_v1.rs");

/// Reduces one exact 64-element `i32` row through LDS and writes from lane zero.
///
/// Admitted inputs have a mathematical sum representable by `i32`, making the
/// device collective's wrapping additions equal to the exact host oracle. The
/// public collective implementation performs one unique LDS publish per lane,
/// uniform publish/read barriers, and a final barrier before scratch reuse.
#[kernel(
    typed,
    namespace = "6bc8f449f458cf8f31b4625b38b7204dd34f20beeabb80b55454a5666be749b5",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn lds_publish_read_reduce_i32_v1(values: &[i32], epoch: u32, mut output: DisjointSlice<i32>) {
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

    let Some(workgroup_size) = WorkgroupSize::new(64, 1, 1) else {
        fe2o3_device::trap();
        return;
    };
    let Some(grid_size) = GridSize::new(1, 1, 1) else {
        fe2o3_device::trap();
        return;
    };
    // SAFETY: the exact source profile checks every physical coordinate and
    // launch extent above. Compiler profile authentication remains required.
    let Some(invocation) = (unsafe {
        Invocation3D::from_raw_parts(
            WorkitemId::new(lane, 0, 0),
            WorkgroupId::new(0, 0, 0),
            workgroup_size,
            grid_size,
        )
    }) else {
        fe2o3_device::trap();
        return;
    };
    let Some(group) = Workgroup::from_invocation_snapshot(&invocation) else {
        fe2o3_device::trap();
        return;
    };

    // SAFETY: registration must bind one aligned 64-slot LDS allocation to
    // this workgroup and epoch. The unregistered helper traps closed today.
    let scratch_base = unsafe { workgroup64_lds_i32_base_v1(epoch) };
    let Ok(mut scratch) =
        (unsafe { WorkgroupCollectiveScratch::from_raw_parts(&group, scratch_base, 64) })
    else {
        fe2o3_device::trap();
        return;
    };
    // SAFETY: the checked source has one physical wave64, and all lanes reach
    // the public LDS collective in uniform control flow.
    let context = unsafe { Gfx942Collectives::from_compiler() };
    let value = values[lane as usize];
    // SAFETY: `group`, `context`, and `scratch` describe the same exact epoch;
    // every lane executes the collective and all of its barriers uniformly.
    let sum = unsafe { group.reduce_sum(&context, &mut scratch, value) };

    if lane == 0 {
        // SAFETY: lane zero is the sole writer to the sole output element.
        if let Some(slot) = unsafe { output.get_mut_at(0) } {
            *slot = sum;
        } else {
            fe2o3_device::trap();
        }
    }
}

/// Future compiler intrinsic for one epoch-branded 64-slot LDS allocation.
///
/// # Safety
///
/// Registered lowering must return the same live, aligned workgroup-address-
/// space allocation to all 64 lanes, isolate workgroups and epochs, and retain
/// it through the collective's final reuse barrier.
#[inline(never)]
unsafe fn workgroup64_lds_i32_base_v1(_epoch: u32) -> *mut i32 {
    unreachable!("workgroup64 LDS base requires authenticated compiler profile registration")
}
