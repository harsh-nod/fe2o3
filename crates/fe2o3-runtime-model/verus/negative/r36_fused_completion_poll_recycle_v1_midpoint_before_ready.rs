// Expected-negative R36 mutation: the profiler midpoint is captured while the
// poll result is still Pending rather than after Ready completion transitions.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum PollV1 { Pending, Ready }
pub struct StateV1 { pub poll: PollV1, pub midpoint: Option<nat> }

pub open spec fn mutated_pending_midpoint_v1() -> StateV1 {
    StateV1 { poll: PollV1::Pending, midpoint: Some(4) }
}

pub proof fn mutated_pending_has_no_midpoint_v1()
    ensures mutated_pending_midpoint_v1().midpoint.is_none(),
{}
}
