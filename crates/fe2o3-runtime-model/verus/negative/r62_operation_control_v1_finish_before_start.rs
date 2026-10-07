// Expected-negative R62 host control mutation: finish_before_start.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_finish_v1(p: nat) -> nat { if p == 0 || p == 2 || p == 3 { 4 } else { p } }
pub proof fn mutated_finish_before_start_v1()
    ensures mutated_finish_v1(0) != 4,
{}
}
