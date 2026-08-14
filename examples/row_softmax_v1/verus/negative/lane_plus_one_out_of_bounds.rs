use vstd::prelude::*;

verus! {

pub open spec fn row_elements_v1() -> nat { 64 }

pub open spec fn mutated_lane_index_v1(lane: nat) -> nat { lane + 1 }

pub proof fn mutated_lane_plus_one_is_bounded_v1(lane: nat)
    requires lane < row_elements_v1(),
    ensures mutated_lane_index_v1(lane) < row_elements_v1(),
{
}

} // verus!
