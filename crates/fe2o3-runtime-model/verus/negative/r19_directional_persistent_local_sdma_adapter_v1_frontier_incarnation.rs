use vstd::prelude::*;
verus! {
pub open spec fn mutated_frontier_matches_v1(allocation: nat, incarnation: nat) -> bool {
    allocation == 1
}
pub proof fn mutated_cross_incarnation_frontier_is_rejected_v1()
    ensures !mutated_frontier_matches_v1(1, 99),
{}
}
