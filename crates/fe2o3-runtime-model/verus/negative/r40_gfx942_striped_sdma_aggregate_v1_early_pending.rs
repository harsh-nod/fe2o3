// Expected-negative R40 mutation: polling returns at the first Pending entry.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_observation_count_v1(first_pending: nat) -> nat { first_pending + 1 }
pub proof fn mutated_pending_observes_entire_roster_v1(first_pending: nat, request_count: nat)
    requires first_pending + 1 < request_count,
    ensures mutated_observation_count_v1(first_pending) == request_count,
{}
}
