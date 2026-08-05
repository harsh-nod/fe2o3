use vstd::prelude::*;

verus! {

/// Mutation: replacing the shared identity index with a constant makes two
/// distinct hardware witnesses select the same output element.
pub open spec fn mutated_real_kernel_index(_thread: nat) -> nat {
    0
}

pub proof fn mutated_real_kernel_index_is_injective(left: nat, right: nat)
    requires
        left != right,
    ensures
        mutated_real_kernel_index(left) != mutated_real_kernel_index(right),
{
}

} // verus!
