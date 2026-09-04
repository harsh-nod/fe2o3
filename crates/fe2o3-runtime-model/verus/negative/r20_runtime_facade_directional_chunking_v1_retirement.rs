use vstd::prelude::*;
verus! {
pub open spec fn mutated_retire_v1(expected: nat, observed: nat, completed: nat) -> nat {
    if expected != observed { completed + 1 } else { completed + 1 }
}
pub proof fn mutated_stale_frontier_retirement_is_atomic_v1()
    ensures mutated_retire_v1(7, 8, 4096) == 4096, {}
}
