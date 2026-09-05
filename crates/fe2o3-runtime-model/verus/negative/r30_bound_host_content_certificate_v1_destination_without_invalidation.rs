use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Host, FullH2dCompleted }

#[derive(PartialEq, Eq)]
pub enum MutationKindV1 { CpuDestination, SdmaDestination }

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub phase: PhaseV1,
    pub certificate: Option<nat>,
    pub transition_clock: nat,
    pub invalidation_step: nat,
    pub possible_mutation_step: nat,
}

// Mutation of the positive destination transition: its ordering ledger remains
// intact, but the live certificate is accidentally retained across mutation.
pub open spec fn mutated_destination_without_invalidation_v1(state: StateV1,
    kind: MutationKindV1) -> StateV1
{
    if state.phase != PhaseV1::Host { state }
    else { StateV1 {
        certificate: state.certificate,
        transition_clock: state.transition_clock + 2,
        invalidation_step: state.transition_clock + 1,
        possible_mutation_step: state.transition_clock + 2,
        ..state
    }}
}

pub proof fn mutated_destination_invalidates_before_possible_mutation_v1(state: StateV1,
    digest: nat)
    requires state.phase == PhaseV1::Host, state.certificate == Some(digest),
    ensures {
        let post = mutated_destination_without_invalidation_v1(
            state, MutationKindV1::CpuDestination);
        &&& post.certificate.is_none()
        &&& post.invalidation_step < post.possible_mutation_step
    }, {}

}
