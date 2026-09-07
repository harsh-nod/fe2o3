// Expected-negative R41 mutation: two requests share one persistent device owner.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_owner_v1(_index: nat) -> nat { 9 }
pub proof fn mutated_device_owners_are_distinct_v1()
    ensures mutated_owner_v1(0) != mutated_owner_v1(1),
{}
}
