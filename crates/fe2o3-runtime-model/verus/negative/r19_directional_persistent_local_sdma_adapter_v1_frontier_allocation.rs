use vstd::prelude::*;
verus! {
pub open spec fn mutated_frontier_matches_v1(allocation: nat, mapping: nat) -> bool {
    mapping == 2
}
pub proof fn mutated_cross_allocation_frontier_is_rejected_v1()
    ensures !mutated_frontier_matches_v1(99, 2),
{}
}
