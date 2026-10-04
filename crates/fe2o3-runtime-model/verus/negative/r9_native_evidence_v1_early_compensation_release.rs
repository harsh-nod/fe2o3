use vstd::prelude::*;

verus! {

pub open spec fn mutated_partial_compensation_releasable_v1(
    unmapped: nat,
    mapped: nat,
) -> bool {
    unmapped < mapped
}

pub proof fn mutated_partial_compensation_blocks_release_v1(unmapped: nat, mapped: nat)
    requires unmapped < mapped,
    ensures !mutated_partial_compensation_releasable_v1(unmapped, mapped),
{
}

} // verus!
