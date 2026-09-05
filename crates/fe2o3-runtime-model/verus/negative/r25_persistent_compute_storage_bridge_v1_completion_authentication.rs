use vstd::prelude::*;
verus! {
pub open spec fn expected_completion_generation_v1() -> nat { 8 }
pub open spec fn mutated_completion_generation_v1() -> nat { 9 }
pub proof fn mutated_completion_coordinates_are_exact_v1()
    ensures mutated_completion_generation_v1() == expected_completion_generation_v1(), {}
}
