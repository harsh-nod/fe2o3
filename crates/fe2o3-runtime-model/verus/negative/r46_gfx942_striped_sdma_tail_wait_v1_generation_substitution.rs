// Expected-negative R46 mutation: tail matching omits queue/signal generation.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_generation_matches_v1(expected: nat, observed: nat) -> bool { true }
pub proof fn mutated_generation_substitution_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_generation_matches_v1(expected, observed),
{}
}
