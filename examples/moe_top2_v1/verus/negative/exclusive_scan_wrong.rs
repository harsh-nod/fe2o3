use vstd::prelude::*;

verus! {

pub open spec fn mutated_offset_v1(expert: nat) -> nat {
    expert * 5
}

pub proof fn mutated_terminal_offset_is_route_bounded_v1()
    ensures mutated_offset_v1(4) <= 16,
{
}

}
