use vstd::prelude::*;

#[path = "../workgroup_sync_v1.rs"]
mod model;

verus! {

pub open spec fn mutated_writes_output_v1(lane: nat) -> bool { lane <= 1 }

pub proof fn mutated_two_output_owners_are_equal_v1(left: nat, right: nat)
    requires
        left < model::lane_count_v1(),
        right < model::lane_count_v1(),
        mutated_writes_output_v1(left),
        mutated_writes_output_v1(right),
    ensures left == right,
{
}

} // verus!
