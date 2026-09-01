use vstd::prelude::*;

verus! {

pub proof fn mutated_vecadd_effect_identity_is_preserved_v1(effect: nat)
    ensures
        effect + 1 == effect,
{
}

} // verus!
