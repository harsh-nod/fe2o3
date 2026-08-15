use vstd::prelude::*;

verus! {

pub open spec fn mutated_denominator_update_v1(old: int, _scale: int, current: int) -> int {
    old + current
}

pub proof fn mutated_denominator_rescales_old_frame_v1()
    ensures mutated_denominator_update_v1(3, 2, 5) == 11,
{
}

}
