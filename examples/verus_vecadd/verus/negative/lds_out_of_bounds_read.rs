use vstd::prelude::*;

verus! {

pub open spec fn element_end(index: nat, element_size: nat) -> nat {
    index * element_size + element_size
}

/// Expected failure marker: mutated_unbounded_lds_read_is_in_bounds.
pub proof fn mutated_unbounded_lds_read_is_in_bounds(
    element_count: nat,
    index: nat,
    element_size: nat,
)
    requires
        element_size > 0,
        index == element_count,
    ensures
        element_end(index, element_size) <= element_count * element_size,
{
}

} // verus!
