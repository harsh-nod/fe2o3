// Expected-negative R57 mutation: the WaitForPrior packet is omitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_wait_omission_is_exact_v1() -> bool { true }
pub proof fn mutated_wait_omission_is_rejected_v1()
    ensures !mutated_wait_omission_is_exact_v1(),
{}
}
fn main() {}
