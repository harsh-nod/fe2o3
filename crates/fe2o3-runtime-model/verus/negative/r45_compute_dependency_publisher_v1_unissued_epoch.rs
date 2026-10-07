// Expected-negative R45 mutation: a coordinated unissued epoch authenticates.
use vstd::prelude::*;
verus! {
pub open spec fn acceptance_epoch_v1() -> nat { 67 }
pub open spec fn issued_epoch_v1() -> nat { 66 }
pub proof fn mutated_unissued_epoch_is_authenticated_v1()
    ensures acceptance_epoch_v1() == issued_epoch_v1(),
{}
}
