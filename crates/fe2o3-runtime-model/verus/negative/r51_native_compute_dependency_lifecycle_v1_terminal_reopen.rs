// Expected-negative R51 mutation: a terminal owner accepts an attempted mutation.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_absorb_v1(before: nat, attempted: nat) -> nat { attempted }
pub proof fn mutated_terminal_is_absorbing_v1(before: nat, attempted: nat)
    ensures mutated_absorb_v1(before, attempted) == before,
{}
}
