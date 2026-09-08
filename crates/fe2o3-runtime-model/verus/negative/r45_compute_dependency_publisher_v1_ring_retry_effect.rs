// Expected-negative R45 mutation: occupancy retry mutates native state.
use vstd::prelude::*;
verus! {
pub enum RingStateV1 { Available, Occupied }
pub open spec fn mutated_ring_state_v1() -> RingStateV1 { RingStateV1::Occupied }
pub open spec fn native_effect_count_v1() -> nat { 1 }
pub proof fn mutated_ring_retry_has_zero_effect_v1()
    ensures mutated_ring_state_v1() == RingStateV1::Occupied
        ==> native_effect_count_v1() == 0,
{}
}
