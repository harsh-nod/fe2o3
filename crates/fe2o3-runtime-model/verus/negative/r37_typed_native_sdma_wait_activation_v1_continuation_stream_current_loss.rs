// Expected-negative R37 mutation: a completed window enters Ready after losing
// its exact current-stream membership.
use vstd::prelude::*;

verus! {
pub struct StateV1 { pub stream_current_retained: bool }

pub open spec fn mutated_continuation_v1() -> StateV1 {
    StateV1 { stream_current_retained: false }
}

pub proof fn mutated_continuation_retains_stream_current_v1()
    ensures mutated_continuation_v1().stream_current_retained,
{}
}
