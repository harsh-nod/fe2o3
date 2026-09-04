use vstd::prelude::*;
verus! {
pub open spec fn mutated_frontier_first_generation_v1(prior: nat) -> nat { prior + 1 }
pub proof fn mutated_frontier_may_substitute_window_roster_v1()
    ensures mutated_frontier_first_generation_v1(21) == 21, {}
}
