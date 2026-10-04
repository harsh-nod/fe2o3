use vstd::prelude::*;

#[path = "../workgroup_sync_v1.rs"]
mod model;

verus! {

pub open spec fn mutated_publish_slot_v1(lane: nat) -> nat { lane + 1 }

pub proof fn mutated_last_lane_still_initializes_in_bounds_v1(lane: nat)
    requires lane < model::lane_count_v1(),
    ensures mutated_publish_slot_v1(lane) < model::lane_count_v1(),
{
}

} // verus!
