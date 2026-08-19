use vstd::prelude::*;

verus! {

pub open spec fn mutated_descriptor_containment_only_v1() -> bool {
    &&& 100 <= 110
    &&& 110 + 4 <= 100 + 64
    &&& 1000 <= 1020
    &&& 1020 + 4 <= 1000 + 64
}

pub proof fn mutated_descriptor_delta_substitution_is_bound_v1()
    requires
        mutated_descriptor_containment_only_v1(),
    ensures
        110nat - 100nat == 1020nat - 1000nat,
{
}

} // verus!
