use vstd::prelude::*;

verus! {

pub struct StateV1 {
    pub current: bool,
    pub published: bool,
    pub owns_resources: bool,
    pub quarantined: bool,
}

pub open spec fn mutated_lose_currentness_v1(state: StateV1) -> StateV1 {
    StateV1 { current: false, ..state }
}

pub proof fn mutated_currentness_loss_quarantines_published_v1(state: StateV1)
    requires
        state.current,
        state.published,
        state.owns_resources,
        !state.quarantined,
    ensures mutated_lose_currentness_v1(state).quarantined,
{
}

}
