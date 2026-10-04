use vstd::prelude::*;

verus! {

pub open spec fn mutated_next_maximum_v1(old: int, next: int) -> int {
    if next < old { next } else { old }
}

pub proof fn mutated_running_max_bounds_next_score_v1()
    ensures mutated_next_maximum_v1(2, 7) >= 7,
{
}

}
