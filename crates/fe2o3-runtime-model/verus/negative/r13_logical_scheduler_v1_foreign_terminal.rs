use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub lane: nat,
    pub terminal: bool,
}

pub open spec fn mutated_observe_terminal_v1(state: StateV1, _lane: nat) -> StateV1 {
    StateV1 { terminal: true, ..state }
}

pub proof fn mutated_foreign_lane_cannot_complete_v1(state: StateV1, foreign_lane: nat)
    requires
        state.lane != foreign_lane,
        !state.terminal,
    ensures mutated_observe_terminal_v1(state, foreign_lane) == state,
{
}

}
