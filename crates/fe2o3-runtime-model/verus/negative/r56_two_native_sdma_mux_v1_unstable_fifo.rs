// Expected-negative R56 mutation: descending issue indices are called stable.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_stable_v1(left: nat, right: nat) -> bool { true }
pub proof fn mutated_unstable_fifo_is_rejected_v1()
    ensures !mutated_stable_v1(2, 1),
{}
}
fn main() {}
