// Expected-negative R37 mutation: an already-expired deadline skips the one
// required native completion observation.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum DeadlineV1 { Zero, Positive }

pub open spec fn mutated_observation_count_v1(deadline: DeadlineV1) -> nat {
    if deadline == DeadlineV1::Zero { 0 } else { 1 }
}

pub proof fn mutated_zero_deadline_observes_once_v1()
    ensures mutated_observation_count_v1(DeadlineV1::Zero) == 1,
{}
}
