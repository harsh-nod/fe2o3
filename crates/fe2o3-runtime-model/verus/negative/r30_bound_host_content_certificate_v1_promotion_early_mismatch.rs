use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { FullH2dCompleted, Ready, TerminalAbsorbed }

#[derive(PartialEq, Eq)]
pub enum CurrentnessV1 { Current, Ambiguous }

#[derive(PartialEq, Eq)]
pub enum TerminalStageV1 { OpeningAmbiguous, ClosingAmbiguous }

#[derive(PartialEq, Eq)]
pub struct CompletionV1 { pub generation: nat }

#[derive(PartialEq, Eq)]
pub struct CustodyV1 {
    pub completion: CompletionV1,
    pub stored_certificate: Option<nat>,
    pub stage: TerminalStageV1,
}

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub phase: PhaseV1,
    pub pending: Option<CompletionV1>,
    pub stored_certificate: Option<nat>,
    pub terminal: Option<CustodyV1>,
    pub retired_generation: nat,
}

// Mutation of the positive promotion transition: candidate mismatch is checked
// before the opening/closing currentness envelope instead of afterward.
pub open spec fn mutated_promote_with_early_mismatch_v1(state: StateV1,
    completion: CompletionV1, candidate_certificate: nat,
    opening: CurrentnessV1, closing: CurrentnessV1) -> StateV1
{
    if state.phase == PhaseV1::TerminalAbsorbed { state }
    else if state.phase != PhaseV1::FullH2dCompleted
        || state.pending != Some(completion) { state }
    else if state.stored_certificate != Some(candidate_certificate) { state }
    else if opening == CurrentnessV1::Ambiguous {
        StateV1 {
            phase: PhaseV1::TerminalAbsorbed,
            pending: None,
            terminal: Some(CustodyV1 { completion,
                stored_certificate: state.stored_certificate,
                stage: TerminalStageV1::OpeningAmbiguous }),
            ..state
        }
    } else if closing == CurrentnessV1::Ambiguous {
        StateV1 {
            phase: PhaseV1::TerminalAbsorbed,
            pending: None,
            terminal: Some(CustodyV1 { completion,
                stored_certificate: state.stored_certificate,
                stage: TerminalStageV1::ClosingAmbiguous }),
            ..state
        }
    } else {
        StateV1 {
            phase: PhaseV1::Ready,
            pending: None,
            retired_generation: completion.generation,
            ..state
        }
    }
}

pub proof fn mutated_closing_ambiguity_precedes_certificate_mismatch_v1(state: StateV1,
    completion: CompletionV1, stored_certificate: nat, substituted_certificate: nat)
    requires state.phase == PhaseV1::FullH2dCompleted,
        state.pending == Some(completion),
        state.stored_certificate == Some(stored_certificate),
        substituted_certificate != stored_certificate,
    ensures {
        let post = mutated_promote_with_early_mismatch_v1(state, completion,
            substituted_certificate, CurrentnessV1::Current, CurrentnessV1::Ambiguous);
        &&& post.phase == PhaseV1::TerminalAbsorbed
        &&& post.terminal == Some(CustodyV1 { completion,
            stored_certificate: state.stored_certificate,
            stage: TerminalStageV1::ClosingAmbiguous })
        &&& post.retired_generation == state.retired_generation
    }, {}

}
