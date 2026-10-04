use vstd::prelude::*;
verus! {
pub open spec fn mutated_frontier_matches_v1(parent: nat, pair: nat, attachment: nat) -> bool {
    parent == 3 && attachment == 5
}
pub proof fn mutated_cross_pair_frontier_is_rejected_v1()
    ensures !mutated_frontier_matches_v1(3, 99, 5),
{}
}
