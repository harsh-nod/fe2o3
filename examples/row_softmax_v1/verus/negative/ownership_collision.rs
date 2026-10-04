use vstd::prelude::*;

verus! {

pub open spec fn mutated_output_owner_v1(lane: nat) -> nat {
    lane / 2
}

pub proof fn mutated_adjacent_lanes_have_distinct_output_owners_v1()
    ensures mutated_output_owner_v1(0) != mutated_output_owner_v1(1),
{
}

}
