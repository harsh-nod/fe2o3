use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub published: bool,
    pub current: bool,
    pub owns_resource: bool,
    pub quarantined: bool,
}

pub open spec fn mutated_lose_currentness_v1(state: StateV1) -> StateV1 {
    StateV1 { current: false, owns_resource: false, ..state }
}

pub proof fn mutated_currentness_loss_quarantines_published_v1(state: StateV1)
    requires state.published, state.current, state.owns_resource, !state.quarantined,
    ensures {
        let lost = mutated_lose_currentness_v1(state);
        lost.owns_resource && lost.quarantined
    },
{
}

}
