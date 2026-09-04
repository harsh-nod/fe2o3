use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub submission_id: nat,
    pub stream_tail: nat,
    pub cancelled: bool,
}

pub open spec fn mutated_cancel_v1(state: StateV1) -> StateV1 {
    StateV1 { cancelled: true, ..state }
}

pub proof fn mutated_non_tail_cancel_is_rejected_v1(state: StateV1)
    requires
        state.submission_id != state.stream_tail,
        !state.cancelled,
    ensures mutated_cancel_v1(state) == state,
{
}

}
