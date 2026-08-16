use vstd::prelude::*;
verus! {
pub proof fn mutated_route_id_is_bounded_v1(token: nat, rank: nat)
    requires token < 8, rank < 2,
    ensures token * 2 + rank + 1 < 16,
{
}
}
