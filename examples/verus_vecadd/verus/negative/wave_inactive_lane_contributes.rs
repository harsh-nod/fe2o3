use vstd::prelude::*;

verus! {

pub open spec fn lane_contribution(value: int, active: bool) -> int {
    if active { value } else { 0 }
}

/// Expected failure marker: mutated_inactive_lane_contributes.
pub proof fn mutated_inactive_lane_contributes(value: int)
    requires
        value != 0,
    ensures
        lane_contribution(value, false) == value,
{
}

} // verus!
