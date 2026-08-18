use vstd::prelude::*;

verus! {

pub proof fn mutated_history_forgets_predecessor_v1(previous: nat)
    requires
        previous > 0,
    ensures
        0 == previous,
{
}

} // verus!
