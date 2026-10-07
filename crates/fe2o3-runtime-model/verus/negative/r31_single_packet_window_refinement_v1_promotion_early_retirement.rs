use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)] pub enum PhaseV1 { Completed, TerminalAbsorbed }
#[derive(PartialEq, Eq)] pub enum TerminalStageV1 { PromotionClosing }

#[derive(PartialEq, Eq)] pub struct CompletionV1 {
    pub transfer_id: nat,
    pub packet_count: nat,
}

pub struct StateV1 {
    pub phase: PhaseV1,
    pub completion: Option<CompletionV1>,
    pub stored_certificate: Option<nat>,
    pub retired_frontiers: nat,
    pub terminal_stage: Option<TerminalStageV1>,
}

// Mutation of the positive promotion transition: the single-packet frontier is
// retired before the closing-currentness result is classified.
pub open spec fn mutated_promote_with_early_retirement_v1(
    state: StateV1, candidate_certificate: nat) -> StateV1
{
    if state.phase != PhaseV1::Completed { state } else {
        StateV1 {
            phase: PhaseV1::TerminalAbsorbed,
            completion: None,
            retired_frontiers: state.retired_frontiers + 1,
            terminal_stage: Some(TerminalStageV1::PromotionClosing),
            ..state
        }
    }
}

pub proof fn mutated_closing_ambiguity_retains_completion_without_retirement_v1(
    state: StateV1, substituted_certificate: nat)
    requires state.phase == PhaseV1::Completed,
        state.completion.is_some(),
        state.stored_certificate != Some(substituted_certificate),
    ensures {
        let post = mutated_promote_with_early_retirement_v1(state, substituted_certificate);
        &&& post.phase == PhaseV1::TerminalAbsorbed
        &&& post.terminal_stage == Some(TerminalStageV1::PromotionClosing)
        &&& post.completion == state.completion
        &&& post.retired_frontiers == state.retired_frontiers
    }, {}

}
