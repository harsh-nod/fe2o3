// Expected-negative R57 mutation: a two-packet transaction advances by one.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_frontier_after_v1(frontier_before: nat) -> nat {
    frontier_before + 1
}
pub proof fn mutated_frontier_one_is_rejected_v1(frontier_before: nat)
    ensures mutated_frontier_after_v1(frontier_before) == frontier_before + 2,
{}
}
fn main() {}
