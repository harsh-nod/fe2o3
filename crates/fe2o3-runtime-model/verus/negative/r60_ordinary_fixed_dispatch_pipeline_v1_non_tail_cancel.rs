// Expected-negative R60 mutation: an unpublished non-tail entry is cancellable.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cancel_v1(unpublished: bool, _tail: bool) -> bool { unpublished }
pub proof fn mutated_non_tail_cancel_is_rejected_v1()
    ensures !mutated_cancel_v1(true, false),
{}
}
