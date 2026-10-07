// Expected-negative R40 mutation: two striped queues are both placed on engine zero.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_engine_v1(slot: nat) -> nat { 0 }
pub proof fn mutated_two_slots_are_balanced_v1()
    ensures mutated_engine_v1(0) == 0 && mutated_engine_v1(1) == 1,
{}
}
