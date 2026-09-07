use vstd::prelude::*;

verus! {
pub struct BorrowedCoordinate<'a> {
    pub coordinate: &'a nat,
}

pub open spec fn mutated_matches_v1(_expected: nat, _observed: nat) -> bool { true }
pub proof fn coordinate_omission_is_rejected_v1(expected: nat, observed: nat)
    requires expected != observed,
    ensures !mutated_matches_v1(expected, observed),
{}
}
