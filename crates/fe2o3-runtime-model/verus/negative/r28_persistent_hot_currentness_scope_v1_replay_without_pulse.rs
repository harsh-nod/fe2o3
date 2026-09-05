use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum ScopePhaseV1 { Active, TerminalAbsorbed }

#[derive(PartialEq, Eq)]
pub enum ControlStateV1 { Attached, DataDetached }

#[derive(PartialEq, Eq)]
pub enum AttemptPhaseV1 { Available, Prepared }

#[derive(PartialEq, Eq)]
pub struct ReplayStateV1 {
    pub scope: ScopePhaseV1,
    pub control: ControlStateV1,
    pub attempt_phase: AttemptPhaseV1,
    pub attempt_present: bool,
    pub exact_next_attempt: bool,
    pub next_attachment: nat,
    pub operational_checkpoints: nat,
}

// Mutation of the positive prepare_replay_v1 transition: every admission guard
// and generation effect is retained, but its fresh operational pulse is dropped.
pub open spec fn mutated_prepare_replay_without_pulse_v1(state: ReplayStateV1)
    -> ReplayStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if state.scope == ScopePhaseV1::Active
        && state.control == ControlStateV1::DataDetached
        && state.attempt_phase == AttemptPhaseV1::Available
        && !state.attempt_present && state.exact_next_attempt
    {
        ReplayStateV1 {
            control: ControlStateV1::Attached,
            attempt_phase: AttemptPhaseV1::Prepared,
            attempt_present: true,
            next_attachment: state.next_attachment + 1,
            operational_checkpoints: state.operational_checkpoints,
            ..state
        }
    } else { state }
}

pub proof fn mutated_replay_has_fresh_operational_pulse_v1(state: ReplayStateV1)
    requires state.scope == ScopePhaseV1::Active,
        state.control == ControlStateV1::DataDetached,
        state.attempt_phase == AttemptPhaseV1::Available,
        !state.attempt_present, state.exact_next_attempt,
    ensures mutated_prepare_replay_without_pulse_v1(state).operational_checkpoints
        == state.operational_checkpoints + 1, {}

}
