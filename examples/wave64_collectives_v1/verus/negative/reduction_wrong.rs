use vstd::prelude::*;

verus! {

pub open spec fn mutated_reduction_v1(correct_reduction: int) -> int {
    correct_reduction + 1
}

pub proof fn mutated_reduction_equals_full_sum_v1(correct_reduction: int)
    ensures mutated_reduction_v1(correct_reduction) == correct_reduction,
{
}

}
