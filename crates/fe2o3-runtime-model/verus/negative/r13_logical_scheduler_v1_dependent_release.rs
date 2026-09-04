use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub has_queued_dependent: bool,
    pub owns_resources: bool,
}

pub open spec fn mutated_release_v1(state: StateV1) -> StateV1 {
    StateV1 { owns_resources: false, ..state }
}

pub proof fn mutated_queued_dependent_retains_terminal_resources_v1(state: StateV1)
    requires
        state.has_queued_dependent,
        state.owns_resources,
    ensures mutated_release_v1(state) == state,
{
}

}
