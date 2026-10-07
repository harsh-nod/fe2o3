use vstd::prelude::*;
verus! {
pub open spec fn mutated_poll_visits_v1() -> nat { 3 }
pub proof fn mutated_poll_budget_is_bounded_v1()
    ensures mutated_poll_visits_v1() <= 2, {}
}
