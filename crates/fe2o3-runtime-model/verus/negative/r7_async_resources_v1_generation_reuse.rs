use vstd::prelude::*;

verus! {

pub proof fn mutated_released_generation_is_reusable_v1(generation: nat)
    requires generation > 0,
    ensures generation == generation + 1,
{
}

} // verus!
