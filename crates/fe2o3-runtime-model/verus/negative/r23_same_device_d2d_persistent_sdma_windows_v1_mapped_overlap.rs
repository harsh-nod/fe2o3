use vstd::prelude::*;
verus! {
pub open spec fn mutated_mapped_ranges_are_disjoint_v1(
    left_start: nat, left_end: nat, right_start: nat, right_end: nat) -> bool
{
    left_start < right_end && right_start < left_end
}
pub proof fn mutated_mapped_overlap_is_rejected_v1()
    ensures !mutated_mapped_ranges_are_disjoint_v1(0, 8, 4, 12), {}
}
