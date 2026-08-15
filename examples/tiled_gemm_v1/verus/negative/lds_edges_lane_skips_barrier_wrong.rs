use vstd::prelude::*;

#[path = "../lds_tiled_edges_alpha_beta.rs"]
mod model;

verus! {

/// Mutation: a lane with a predicated-off A load skips the publish barrier.
pub open spec fn mutated_lane_reaches_publish_barrier_v1(
    lane: nat,
    phase: nat,
    phase_count: nat,
    a_load_enabled: bool,
) -> bool {
    lane < 64 && phase < phase_count && a_load_enabled
}

pub proof fn mutated_predicate_off_lane_still_reaches_barrier_v1()
    ensures mutated_lane_reaches_publish_barrier_v1(63, 0, 1, false),
{
}

} // verus!
