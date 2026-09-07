// Expected-negative R46 mutation: tail submission identity is not checked.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_tail_matches_v1(expected_submission: nat, observed_submission: nat) -> bool {
    true
}
pub proof fn mutated_tail_substitution_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_tail_matches_v1(expected, observed),
{}
}
