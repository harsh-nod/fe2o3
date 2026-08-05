use vstd::prelude::*;

verus! {

/// Expected failure: this mutation maps every thread to output element zero.
pub open spec fn mutated_output_index(_thread: nat) -> nat {
    0
}

pub proof fn mutated_distinct_threads_have_disjoint_outputs(left: nat, right: nat)
    requires
        left != right,
    ensures
        mutated_output_index(left) != mutated_output_index(right),
{
}

} // verus!
