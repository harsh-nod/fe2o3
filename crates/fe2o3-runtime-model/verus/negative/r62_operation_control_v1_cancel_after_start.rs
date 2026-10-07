// Expected-negative R62 host control mutation: cancel_after_start.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cancel_v1(p: nat) -> nat { if p == 0 || p == 2 { 1 } else { p } }
pub proof fn mutated_cancel_after_start_v1()
    ensures mutated_cancel_v1(2) == 2,
{}
}
