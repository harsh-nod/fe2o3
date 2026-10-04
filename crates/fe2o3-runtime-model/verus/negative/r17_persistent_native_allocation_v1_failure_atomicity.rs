use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)]
pub struct StateV1 { pub occupied: nat, pub generation: nat }
pub open spec fn mutated_failed_reservation_v1(state: StateV1) -> StateV1 {
    StateV1 { occupied: state.occupied + 1, generation: state.generation + 1 }
}
pub proof fn mutated_failed_reservation_preserves_state_v1(state: StateV1)
    ensures mutated_failed_reservation_v1(state) == state,
{}
}
