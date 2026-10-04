use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub published: bool,
    pub owns_slot: bool,
    pub owns_resource: bool,
}

pub open spec fn mutated_cancel_v1(state: StateV1) -> StateV1 {
    StateV1 { owns_slot: false, owns_resource: false, ..state }
}

pub proof fn mutated_published_cancellation_retains_custody_v1(state: StateV1)
    requires state.published, state.owns_slot, state.owns_resource,
    ensures mutated_cancel_v1(state) == state,
{
}

}
