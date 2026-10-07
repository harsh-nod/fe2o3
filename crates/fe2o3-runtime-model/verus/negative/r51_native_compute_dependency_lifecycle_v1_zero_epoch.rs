// Expected-negative R51 mutation: an accepted target carries epoch zero.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_accepted_epoch_v1() -> nat { 0 }
pub proof fn mutated_accepted_epoch_is_nonzero_v1()
    ensures mutated_accepted_epoch_v1() > 0,
{}
}
