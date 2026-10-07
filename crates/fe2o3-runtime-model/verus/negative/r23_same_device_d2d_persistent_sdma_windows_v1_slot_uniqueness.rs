use vstd::prelude::*;
verus! {
pub open spec fn mutated_slot_v1(_index: nat) -> nat { 0 }
pub proof fn mutated_distinct_d2d_packets_use_unique_slots_v1()
    ensures mutated_slot_v1(0) != mutated_slot_v1(1), {}
}
