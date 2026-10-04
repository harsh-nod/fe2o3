use vstd::prelude::*;

verus! {

pub open spec fn alias_domain(address_space: nat) -> nat {
    if address_space == 0 || address_space == 1 || address_space == 4 {
        0
    } else {
        address_space
    }
}

pub open spec fn physical_ranges_overlap(
    left_space: nat,
    left_start: nat,
    left_len: nat,
    right_space: nat,
    right_start: nat,
    right_len: nat,
) -> bool {
    alias_domain(left_space) == alias_domain(right_space)
        && left_len > 0
        && right_len > 0
        && left_start < right_start + right_len
        && right_start < left_start + left_len
}

pub proof fn mutated_global_and_flat_tags_imply_physical_disjointness()
    ensures
        !physical_ranges_overlap(1, 0x1000, 16, 0, 0x1000, 16),
{
}

} // verus!
