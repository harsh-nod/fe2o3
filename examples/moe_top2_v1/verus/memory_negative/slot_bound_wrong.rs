use vstd::prelude::*;
verus! {
pub proof fn mutated_accepted_slot_is_bounded_v1(offset: nat, rank: nat)
    requires offset <= 16, rank < 4, offset + rank < 17,
    ensures offset + rank < 16,
{
}
}
