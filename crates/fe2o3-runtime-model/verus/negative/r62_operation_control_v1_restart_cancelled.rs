// Expected-negative R62 host control mutation: restart_cancelled.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_start_v1(p: nat) -> nat { if p == 0 || p == 1 { 2 } else { p } }
pub proof fn mutated_restart_cancelled_v1()
    ensures mutated_start_v1(1) == 1,
{}
}
