use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum MutatedUnmapStateV1 {
    UnmapPending,
    Unmapped,
    Ambiguous,
}

pub open spec fn mutated_failed_full_unmap_v1(
    mapped_end: nat,
    n_success: nat,
) -> MutatedUnmapStateV1 {
    if n_success == mapped_end {
        MutatedUnmapStateV1::Unmapped
    } else {
        MutatedUnmapStateV1::Ambiguous
    }
}

pub proof fn mutated_failed_full_unmap_is_unreleasable_v1(mapped_end: nat)
    requires
        mapped_end > 0,
    ensures
        mutated_failed_full_unmap_v1(mapped_end, mapped_end)
            != MutatedUnmapStateV1::Unmapped,
{
}

} // verus!
