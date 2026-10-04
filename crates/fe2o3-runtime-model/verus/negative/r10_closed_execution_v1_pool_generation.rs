use vstd::prelude::*;

verus! {

pub open spec fn mutated_release_generation_v1(generation: nat) -> nat {
    generation
}

pub proof fn mutated_completed_pool_release_advances_generation_v1(generation: nat)
    requires generation > 0,
    ensures mutated_release_generation_v1(generation) == generation + 1,
{
}

} // verus!
