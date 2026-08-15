use vstd::prelude::*;

verus! {

pub open spec fn mutated_output_index_v1(lane: nat) -> nat { lane + 1 }

pub proof fn mutated_lane_63_output_is_bounded_v1()
    ensures mutated_output_index_v1(63) < 64,
{
}

}
