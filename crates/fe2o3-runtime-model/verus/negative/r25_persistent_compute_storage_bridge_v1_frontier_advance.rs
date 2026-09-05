use vstd::prelude::*;
verus! {
pub open spec fn completed_generation_v1() -> nat { 14 }
pub open spec fn mutated_retirement_generation_v1() -> nat { 13 }
pub proof fn mutated_retirement_advances_exact_frontier_v1()
    ensures mutated_retirement_generation_v1() == completed_generation_v1(), {}
}
