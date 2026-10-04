use vstd::prelude::*;
verus! {
pub proof fn mutated_combined_index_is_bounded_v1(token: nat, output: nat)
    requires token < 8, output < 16,
    ensures token * 16 + output + 1 < 128,
{
}
}
