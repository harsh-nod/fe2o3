//! Ordinary attributed Rust source for the fixed masked Wave64 collectives.

#![allow(missing_docs)] // The V1 kernel macro emits an undocumented helper module.

use fe2o3_device::{
    DisjointSlice, Gfx942Collectives, SubgroupTile, Wave64, WaveLane, kernel, thread,
};

/// Exact launch dimensions required by the Phase A source contract.
pub const WAVE64_COLLECTIVES_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

/// Computes one masked sum reduction plus inclusive and exclusive scans.
///
/// The `u64` mask is logical participation data. Every physical lane follows
/// the same three collective calls. Inactive lanes contribute positive zero
/// and publish positive zero; active lanes publish the full reduction and
/// their increasing-lane-order prefixes.
#[kernel(
    typed,
    namespace = "2863304ebf7f501a7f177c5b8f5a456261ee34760472727ba3f0205ccf5ce9cc",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn wave64_collectives_v1(
    input: &[f32],
    active_mask: u64,
    mut reduction_output: DisjointSlice<f32>,
    mut inclusive_output: DisjointSlice<f32>,
    mut exclusive_output: DisjointSlice<f32>,
) {
    let lane = thread::index_1d().get();
    if lane >= 64
        || input.len() != 64
        || reduction_output.len() != 64
        || inclusive_output.len() != 64
        || exclusive_output.len() != 64
    {
        fe2o3_device::trap();
        return;
    }

    let active = active_mask & (1_u64 << lane) != 0;
    let contribution = if active { input[lane] } else { 0.0_f32 };

    // SAFETY: the exact launch contract fixes this index to one physical
    // Wave64 lane, and the check above excludes every other value.
    let Some(lane_snapshot) = (unsafe { WaveLane::<Wave64>::from_raw(lane as u32) }) else {
        fe2o3_device::trap();
        return;
    };
    let wave = SubgroupTile::<64>::from_wave64_snapshot(&lane_snapshot);

    // SAFETY: authenticated lowering may create this capability only for the
    // declared gfx942 strict-FP Wave64 profile. Phase A does not claim that
    // compiler authentication has been joined to this source.
    let context = unsafe { Gfx942Collectives::from_compiler() };

    // SAFETY: all 64 physical lanes execute these calls in identical order.
    // Logical inactivity is represented only by a +0.0 contribution.
    let reduction = unsafe { wave.reduce_sum(&context, contribution) };
    let inclusive = unsafe { wave.inclusive_scan_sum(&context, contribution) };
    let exclusive = unsafe { wave.exclusive_scan_sum(&context, contribution) };

    let published_reduction = if active { reduction } else { 0.0 };
    let published_inclusive = if active { inclusive } else { 0.0 };
    let published_exclusive = if active { exclusive } else { 0.0 };

    // SAFETY: lane identity is injective over `0..64`, and each view denotes a
    // distinct output allocation supplied under the typed kernel contract.
    if let Some(output) = unsafe { reduction_output.get_mut_at(lane) } {
        *output = published_reduction;
    }
    // SAFETY: the same identity ownership argument applies to this allocation.
    if let Some(output) = unsafe { inclusive_output.get_mut_at(lane) } {
        *output = published_inclusive;
    }
    // SAFETY: the same identity ownership argument applies to this allocation.
    if let Some(output) = unsafe { exclusive_output.get_mut_at(lane) } {
        *output = published_exclusive;
    }
}
