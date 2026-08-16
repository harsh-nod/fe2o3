use vstd::prelude::*;
verus! {
pub proof fn mutated_weight_index_is_bounded_v1(expert: nat, depth: nat, output: nat)
    requires expert < 4, depth < 16, output < 16,
    ensures expert * 256 + depth * 16 + output + 1 < 1024,
{
}
}
