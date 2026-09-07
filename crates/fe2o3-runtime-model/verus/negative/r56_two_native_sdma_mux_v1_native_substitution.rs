// Expected-negative R56 mutation: ticket validation ignores native ordinal.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_matches_v1(expected: nat, observed: nat) -> bool { true }
pub proof fn mutated_native_substitution_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_matches_v1(expected, observed),
{}
}
fn main() {}
