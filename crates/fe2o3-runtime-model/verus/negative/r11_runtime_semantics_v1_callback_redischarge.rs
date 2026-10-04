use vstd::prelude::*;

verus! {

pub proof fn mutated_repeated_completion_preserves_callback_count_v1(registered: nat)
    requires registered > 0,
    ensures registered + 1 == registered,
{
}

} // verus!
