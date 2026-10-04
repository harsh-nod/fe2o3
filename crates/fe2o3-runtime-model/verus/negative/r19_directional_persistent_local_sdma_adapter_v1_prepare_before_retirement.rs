use vstd::prelude::*;
verus! {
pub struct StateV1 { pub idle: bool, pub occupied: nat, pub frontier_pending: bool }
pub open spec fn mutated_can_prepare_v1(state: StateV1) -> bool {
    state.occupied <= 1
}
pub proof fn mutated_frontier_must_retire_before_prepare_v1()
    ensures !mutated_can_prepare_v1(
        StateV1 { idle: false, occupied: 1, frontier_pending: true }),
{}
}
