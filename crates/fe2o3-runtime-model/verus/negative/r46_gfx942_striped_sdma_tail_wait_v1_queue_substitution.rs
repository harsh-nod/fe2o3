// Expected-negative R46 mutation: tail matching omits queue identity.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_queue_matches_v1(expected_queue: nat, observed_queue: nat) -> bool { true }
pub proof fn mutated_queue_substitution_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_queue_matches_v1(expected, observed),
{}
}
