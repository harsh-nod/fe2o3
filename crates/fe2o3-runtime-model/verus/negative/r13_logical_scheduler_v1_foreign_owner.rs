use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub submission_id: nat,
    pub lane: nat,
    pub terminal: bool,
}

pub open spec fn mutated_observe_terminal_v1(
    state: StateV1,
    observed_lane: nat,
    _observed_owner: nat,
) -> StateV1 {
    if state.lane == observed_lane {
        StateV1 { terminal: true, ..state }
    } else {
        state
    }
}

pub proof fn mutated_foreign_lane_owner_cannot_complete_v1(
    state: StateV1,
    foreign_owner: nat,
)
    requires
        state.submission_id != foreign_owner,
        !state.terminal,
    ensures mutated_observe_terminal_v1(state, state.lane, foreign_owner) == state,
{
}

}
