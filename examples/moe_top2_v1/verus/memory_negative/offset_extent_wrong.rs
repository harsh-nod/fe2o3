use vstd::prelude::*;
verus! {
pub proof fn mutated_offset_index_is_bounded_v1(expert: nat)
    requires expert < 4,
    ensures expert + 1 < 4,
{
}
}
