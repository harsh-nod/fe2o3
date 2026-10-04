use vstd::prelude::*;
verus! {
pub open spec fn mutated_slot_v1(_index: nat) -> nat { 7 }
pub proof fn mutated_distinct_packets_may_share_a_slot_v1()
    ensures mutated_slot_v1(0) != mutated_slot_v1(1), {}
}
