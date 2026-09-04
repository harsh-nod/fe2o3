use vstd::prelude::*;
verus! {
pub open spec fn mutated_flush_visits_v1() -> nat { 2 }
pub proof fn mutated_flush_budget_is_bounded_v1()
    ensures mutated_flush_visits_v1() <= 1, {}
}
