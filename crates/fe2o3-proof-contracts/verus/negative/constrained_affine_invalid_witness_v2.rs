use vstd::prelude::*;

verus! {

/// Hostile mutation: witness zero does not satisfy the retained row `2-x<=0`.
pub proof fn invalid_domain_witness_is_not_sound()
    ensures 2 - 0 <= 0,
{
}

}
