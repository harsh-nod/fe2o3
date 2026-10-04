use vstd::prelude::*;

#[path = "../workgroup_sync_v1.rs"]
mod model;

verus! {

pub open spec fn mutated_publish_barrier_v1(epoch: nat, lane: nat) -> nat {
    model::publish_barrier_v1(epoch) + lane
}

pub proof fn mutated_distinct_lanes_reach_same_barrier_v1(
    epoch: nat,
    left: nat,
    right: nat,
)
    requires
        left < model::lane_count_v1(),
        right < model::lane_count_v1(),
        left != right,
    ensures
        mutated_publish_barrier_v1(epoch, left)
            == mutated_publish_barrier_v1(epoch, right),
{
}

} // verus!
