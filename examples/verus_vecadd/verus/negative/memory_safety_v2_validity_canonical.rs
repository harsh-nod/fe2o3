use vstd::prelude::*;

verus! {

pub open spec fn canonical_validity_pair(
    left_start: nat,
    left_end: nat,
    right_start: nat,
    right_end: nat,
) -> bool {
    left_start <= left_end && right_start <= right_end && left_end + 1 < right_start
}

pub proof fn mutated_adjacent_validity_ranges_are_canonical()
    ensures
        canonical_validity_pair(2, 3, 4, 5),
{
}

} // verus!
