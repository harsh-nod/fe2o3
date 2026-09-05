use vstd::prelude::*;
verus! {
pub open spec fn active_frontier_v1() -> nat { 11 }
pub open spec fn mutated_retired_frontier_v1() -> nat { 10 }
pub proof fn mutated_frontier_retirement_is_exact_v1()
    ensures mutated_retired_frontier_v1() == active_frontier_v1(), {}
}
