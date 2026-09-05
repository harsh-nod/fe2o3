use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)] pub enum DirectionV1 { HostToDevice, DeviceToHost }
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Ready, TerminalAbsorbed }
#[derive(PartialEq, Eq)] pub enum TerminalStageV1 { SubmitClosing }

pub struct StateV1 {
    pub direction: DirectionV1,
    pub phase: PhaseV1,
    pub certificate: Option<nat>,
    pub certificate_invalidated: bool,
    pub host_destination_may_have_mutated: bool,
    pub retired_frontiers: nat,
    pub terminal_stage: Option<TerminalStageV1>,
}

// Mutation of the positive closing-ambiguity transition: the D2H destination
// can have been published, but the terminal state retains its certificate and
// reports that host memory could not have changed.
pub open spec fn mutated_closing_d2h_without_invalidation_v1(state: StateV1) -> StateV1 {
    if state.phase != PhaseV1::Ready { state } else {
        StateV1 {
            phase: PhaseV1::TerminalAbsorbed,
            terminal_stage: Some(TerminalStageV1::SubmitClosing),
            ..state
        }
    }
}

pub proof fn mutated_d2h_closing_ambiguity_invalidates_before_possible_mutation_v1(
    state: StateV1)
    requires state.phase == PhaseV1::Ready,
        state.direction == DirectionV1::DeviceToHost,
        state.certificate.is_some(),
        !state.certificate_invalidated,
        !state.host_destination_may_have_mutated,
    ensures {
        let post = mutated_closing_d2h_without_invalidation_v1(state);
        &&& post.phase == PhaseV1::TerminalAbsorbed
        &&& post.terminal_stage == Some(TerminalStageV1::SubmitClosing)
        &&& post.certificate.is_none()
        &&& post.certificate_invalidated
        &&& post.host_destination_may_have_mutated
        &&& post.retired_frontiers == state.retired_frontiers
    }, {}

}
