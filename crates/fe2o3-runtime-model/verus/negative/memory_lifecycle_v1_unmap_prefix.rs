use vstd::prelude::*;

verus! {

pub struct MutatedProgressV1 {
    pub mapped_start: nat,
    pub mapped_end: nat,
}

pub open spec fn mutated_unmap_drops_unreported_suffix_v1(
    old: MutatedProgressV1,
    n_success: nat,
) -> MutatedProgressV1 {
    MutatedProgressV1 {
        mapped_start: old.mapped_start + n_success,
        mapped_end: old.mapped_start + n_success,
    }
}

pub proof fn mutated_unmap_retains_unreported_suffix_v1(
    old: MutatedProgressV1,
    n_success: nat,
)
    requires
        old.mapped_start < old.mapped_end,
        n_success < old.mapped_end - old.mapped_start,
    ensures
        mutated_unmap_drops_unreported_suffix_v1(old, n_success).mapped_end
            == old.mapped_end,
{
}

} // verus!
