// Expected-negative R56 mutation: tail validation ignores the packet coordinate.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_tail_matches_v1(expected_packet: nat, observed_packet: nat) -> bool { true }
pub proof fn mutated_tail_substitution_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_tail_matches_v1(expected, observed),
{}
}
fn main() {}
