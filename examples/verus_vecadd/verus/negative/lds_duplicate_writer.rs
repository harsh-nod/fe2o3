use vstd::prelude::*;

verus! {

pub open spec fn mutated_write_offset(_thread: nat, _element_size: nat) -> nat {
    0
}

pub open spec fn ranges_overlap(
    left_offset: nat,
    right_offset: nat,
    element_size: nat,
) -> bool {
    left_offset < right_offset + element_size
        && right_offset < left_offset + element_size
}

/// Expected failure marker: mutated_duplicate_lds_writers_are_race_free.
pub proof fn mutated_duplicate_lds_writers_are_race_free(
    left: nat,
    right: nat,
    element_size: nat,
)
    requires
        left != right,
        element_size > 0,
    ensures
        !ranges_overlap(
            mutated_write_offset(left, element_size),
            mutated_write_offset(right, element_size),
            element_size,
        ),
{
}

} // verus!
