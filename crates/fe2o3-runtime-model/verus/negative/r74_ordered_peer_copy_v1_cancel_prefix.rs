// Expected-negative R74 mutation: ticket absence incorrectly permits cancellation.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cancel_v1(ticket: bool, ever: bool) -> bool { !ticket }
pub proof fn mutated_cancel_prefix_v1()
    ensures !mutated_cancel_v1(false, true),
{}
}
