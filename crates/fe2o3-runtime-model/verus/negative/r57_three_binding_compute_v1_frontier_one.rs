// Expected-negative R57 mutation: the two-packet transaction advances by one.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_frontier_one_is_exact_v1() -> bool { true }
pub proof fn mutated_frontier_one_is_rejected_v1()
    ensures !mutated_frontier_one_is_exact_v1(),
{}
}
fn main() {}
