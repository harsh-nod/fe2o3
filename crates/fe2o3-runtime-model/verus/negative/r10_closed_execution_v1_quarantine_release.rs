use vstd::prelude::*;

verus! {

pub open spec fn mutated_quarantined_operation_releasable_v1(quarantined: bool) -> bool {
    quarantined
}

pub proof fn mutated_indeterminate_failure_blocks_release_v1()
    ensures !mutated_quarantined_operation_releasable_v1(true),
{
}

} // verus!
