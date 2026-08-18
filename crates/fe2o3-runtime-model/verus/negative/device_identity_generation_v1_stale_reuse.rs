use vstd::prelude::*;

verus! {

pub open spec fn generation_advances_v1(previous: nat, requested: nat) -> bool {
    previous < requested
}

pub proof fn mutated_stale_generation_reuse_advances_v1()
    ensures
        generation_advances_v1(7, 7),
{
}

} // verus!
