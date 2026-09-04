use vstd::prelude::*;

verus! {

pub open spec fn mutated_collective_admitted_v1(participants: nat, expected: nat) -> bool {
    participants > 0
}

pub proof fn mutated_collective_membership_mismatch_is_rejected_v1(
    participants: nat,
    expected: nat,
)
    requires participants > 0, participants != expected,
    ensures !mutated_collective_admitted_v1(participants, expected),
{
}

} // verus!
