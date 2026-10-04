use vstd::prelude::*;

verus! {

pub open spec fn mutated_output_owner_v1(_lane: nat) -> nat { 0 }

pub proof fn mutated_distinct_lanes_have_distinct_owners_v1()
    ensures mutated_output_owner_v1(0) != mutated_output_owner_v1(1),
{
}

}
