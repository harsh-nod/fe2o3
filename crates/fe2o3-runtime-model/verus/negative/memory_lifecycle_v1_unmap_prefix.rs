use vstd::prelude::*;

verus! {

pub struct MutatedProgressV1 {
    pub mapped_start: nat,
    pub mapped_end: nat,
}

pub open spec fn mutated_unmap_adds_cumulative_progress_v1(
    old: MutatedProgressV1,
    n_success: nat,
) -> MutatedProgressV1 {
    MutatedProgressV1 {
        mapped_start: old.mapped_start + n_success,
        mapped_end: old.mapped_end,
    }
}

pub proof fn mutated_unmap_uses_absolute_cumulative_progress_v1(
    old: MutatedProgressV1,
    n_success: nat,
)
    requires
        0 < old.mapped_start <= n_success < old.mapped_end,
        old.mapped_start + n_success <= old.mapped_end,
    ensures
        mutated_unmap_adds_cumulative_progress_v1(old, n_success).mapped_start
            == n_success,
{
}

} // verus!
