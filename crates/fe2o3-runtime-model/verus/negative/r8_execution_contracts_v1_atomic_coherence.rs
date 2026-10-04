use vstd::prelude::*;

verus! {

pub open spec fn mutated_fetch_add_is_coherent_v1() -> bool {
    false
}

pub proof fn mutated_fetch_add_retains_coherence_v1()
    ensures mutated_fetch_add_is_coherent_v1(),
{
}

} // verus!
