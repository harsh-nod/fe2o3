use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub submission_id: nat,
    pub stream_head: nat,
    pub published: bool,
}

pub open spec fn mutated_publish_v1(state: StateV1) -> StateV1 {
    StateV1 { published: true, ..state }
}

pub proof fn mutated_non_head_cannot_publish_v1(state: StateV1)
    requires
        state.submission_id != state.stream_head,
        !state.published,
    ensures mutated_publish_v1(state) == state,
{
}

}
