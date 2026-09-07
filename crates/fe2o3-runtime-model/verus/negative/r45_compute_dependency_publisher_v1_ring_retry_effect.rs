// Expected-negative R45 mutation: occupancy retry mutates native state.
use vstd::prelude::*;
verus! {
pub open spec fn ring_occupied_v1() -> bool { true }
pub open spec fn native_effect_count_v1() -> nat { 1 }
pub proof fn mutated_ring_retry_has_zero_effect_v1()
    ensures ring_occupied_v1() ==> native_effect_count_v1() == 0,
{}
}
