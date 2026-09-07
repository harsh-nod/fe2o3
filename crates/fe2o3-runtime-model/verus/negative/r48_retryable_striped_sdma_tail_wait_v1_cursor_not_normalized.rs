// Expected-negative R48 mutation: an actual queue ordinal is used as a normalized slot.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_normalized_slot_v1(_first: nat, _queues: nat, queue: nat) -> nat {
    queue
}
pub proof fn rotated_queue_is_normalized_v1()
    ensures mutated_normalized_slot_v1(5, 8, 6) == 1,
{}
}
