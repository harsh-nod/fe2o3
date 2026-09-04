use vstd::prelude::*;
verus! {
pub open spec fn mutated_retire_v1(expected: nat, observed: nat) -> bool { true }
pub proof fn mutated_stale_frontier_is_rejected_v1()
    ensures !mutated_retire_v1(1, 2),
{}
}
