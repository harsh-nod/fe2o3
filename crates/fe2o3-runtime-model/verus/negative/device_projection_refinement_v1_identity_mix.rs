use vstd::prelude::*;

verus! {

pub proof fn mutated_cross_source_identity_mix_is_equal_v1(topology: nat)
    ensures
        topology == topology + 1,
{
}

} // verus!
