// Expected-negative R62 host control mutation: reopen_queued.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_reopen_v1(p: nat) -> nat { if p == 6 { 0 } else { p } }
pub proof fn mutated_reopen_queued_v1()
    ensures mutated_reopen_v1(6) != 0,
{}
}
