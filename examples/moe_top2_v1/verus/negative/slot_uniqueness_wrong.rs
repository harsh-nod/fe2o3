use vstd::prelude::*;

verus! {

pub open spec fn mutated_route_slot_v1(_route: nat) -> nat {
    0
}

pub proof fn mutated_accepted_slots_are_unique_v1()
    ensures mutated_route_slot_v1(0) != mutated_route_slot_v1(1),
{
}

}
