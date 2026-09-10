// Expected-negative R62 host control mutation: repeat_start.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_starts_v1(p: nat, starts: nat) -> nat { if p == 0 || p == 2 { starts + 1 } else { starts } }
pub proof fn mutated_repeat_start_v1()
    ensures mutated_starts_v1(2, 1) <= 1,
{}
}
