use vstd::prelude::*;

verus! {

pub open spec fn mutated_complete_compensation_prefix_v1(mapped: nat) -> nat {
    mapped + 1
}

pub proof fn mutated_complete_compensation_releases_exact_prefix_v1(mapped: nat)
    requires mapped > 0,
    ensures mutated_complete_compensation_prefix_v1(mapped) == mapped,
{
}

} // verus!
