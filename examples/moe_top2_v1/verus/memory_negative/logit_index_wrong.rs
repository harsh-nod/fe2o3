use vstd::prelude::*;
verus! {
pub proof fn mutated_logit_index_is_bounded_v1(token: nat, expert: nat)
    requires token < 8, expert < 4,
    ensures token * 4 + expert < 31,
{
}
}
