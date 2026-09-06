// Expected-negative R38 mutation: the wait increments once beyond its finite
// observation maximum after the final Pending result.
use vstd::prelude::*;

verus! {
pub open spec fn mutated_observations_v1(observation_max: nat) -> nat {
    observation_max + 1
}

pub proof fn mutated_observation_count_respects_maximum_v1(observation_max: nat)
    requires observation_max > 0,
    ensures mutated_observations_v1(observation_max) <= observation_max,
{}
}
