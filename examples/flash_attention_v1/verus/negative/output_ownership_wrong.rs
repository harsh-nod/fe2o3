use vstd::prelude::*;

verus! {

pub open spec fn mutated_lane_output_v1(lane: nat, _slot: nat) -> nat {
    lane * 2
}

pub proof fn mutated_lane_slots_have_distinct_outputs_v1()
    ensures mutated_lane_output_v1(9, 0) != mutated_lane_output_v1(9, 1),
{
}

}
