use vstd::prelude::*;

verus! {

pub open spec fn mutated_published_generation_v1(generation: nat) -> nat {
    generation + 1
}

pub proof fn mutated_ready_publication_retains_generation_v1(generation: nat)
    requires generation > 0,
    ensures mutated_published_generation_v1(generation) == generation,
{
}

} // verus!
