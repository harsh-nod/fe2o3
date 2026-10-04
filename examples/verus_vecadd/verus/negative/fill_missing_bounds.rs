use vstd::prelude::*;

verus! {

pub open spec fn output_index(thread: nat) -> nat {
    thread
}

/// Expected failure: the mutation removed `thread < output.len()`.
pub proof fn mutated_fill_index_is_in_bounds(output: Seq<int>, thread: nat)
    ensures
        output_index(thread) < output.len(),
{
}

} // verus!
