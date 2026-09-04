use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub dependency_succeeded: bool,
    pub published: bool,
}

pub open spec fn mutated_publish_v1(state: StateV1) -> StateV1 {
    StateV1 { published: true, ..state }
}

pub proof fn mutated_unready_dependency_blocks_publication_v1(state: StateV1)
    requires
        !state.dependency_succeeded,
        !state.published,
    ensures mutated_publish_v1(state) == state,
{
}

}
