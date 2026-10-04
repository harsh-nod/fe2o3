use vstd::prelude::*;

verus! {

pub open spec fn ranges_overlap_v1(
    left_start: nat,
    left_end: nat,
    right_start: nat,
    right_end: nat,
) -> bool {
    left_start < right_end && right_start < left_end
}

pub proof fn mutated_memory_only_check_rejects_page_overlap_v1()
    requires
        !ranges_overlap_v1(1, 2, 4095, 4096),
    ensures
        !ranges_overlap_v1(0, 4096, 0, 4096),
{
}

} // verus!
