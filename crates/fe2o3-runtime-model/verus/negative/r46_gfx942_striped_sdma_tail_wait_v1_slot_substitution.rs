// Expected-negative R46 mutation: tail matching omits queue slot.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_slot_matches_v1(expected_slot: nat, observed_slot: nat) -> bool { true }
pub proof fn mutated_slot_substitution_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_slot_matches_v1(expected, observed),
{}
}
