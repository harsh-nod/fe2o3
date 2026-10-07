// Expected-negative R39 mutation: an expired deadline is checked before the
// first completion observation.
use vstd::prelude::*;

verus! {
pub open spec fn mutated_zero_deadline_observations_v1() -> nat { 0 }

pub proof fn mutated_zero_deadline_observes_once_v1()
    ensures mutated_zero_deadline_observations_v1() == 1,
{}
}
