use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub slot: nat,
    pub published: bool,
    pub terminal: bool,
}

pub open spec fn mutated_observe_terminal_v1(state: StateV1, _observed_slot: nat) -> StateV1 {
    if state.published { StateV1 { terminal: true, ..state } } else { state }
}

pub proof fn mutated_cross_queue_terminal_is_rejected_v1(state: StateV1, foreign_slot: nat)
    requires state.published, !state.terminal, foreign_slot != state.slot,
    ensures mutated_observe_terminal_v1(state, foreign_slot) == state,
{
}

}
