use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum CurrentnessV1 { Current, Ambiguous }

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub certificate: Option<nat>,
    pub transition_clock: nat,
    pub invalidation_step: nat,
    pub possible_mutation_step: nat,
}

// Mutation of the positive full-write transition: opening ambiguity returns
// before the production-ordered certificate clear.
pub open spec fn mutated_full_write_without_precheck_clear_v1(state: StateV1,
    opening: CurrentnessV1) -> StateV1
{
    if opening == CurrentnessV1::Ambiguous { state }
    else { StateV1 {
        certificate: None,
        transition_clock: state.transition_clock + 2,
        invalidation_step: state.transition_clock + 1,
        possible_mutation_step: state.transition_clock + 2,
    }}
}

pub proof fn mutated_opening_ambiguity_clears_before_currentness_v1(state: StateV1,
    digest: nat)
    requires state.certificate == Some(digest),
    ensures {
        let post = mutated_full_write_without_precheck_clear_v1(
            state, CurrentnessV1::Ambiguous);
        &&& post.certificate.is_none()
        &&& post.invalidation_step == state.transition_clock + 1
        &&& post.possible_mutation_step == 0
    }, {}

}
