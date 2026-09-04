use vstd::prelude::*;

verus! {

pub open spec fn mutated_begin_mapped_prefix_v1() -> nat {
    1
}

pub proof fn mutated_mapping_begins_with_zero_prefix_v1()
    ensures mutated_begin_mapped_prefix_v1() == 0,
{
}

} // verus!
