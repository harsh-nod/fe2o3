use vstd::prelude::*;

verus! {

pub open spec fn mutated_full_mask_v1(lane: nat) -> bool {
    lane != 63
}

pub proof fn mutated_activity_mask_keeps_lane_63_active_v1()
    ensures mutated_full_mask_v1(63),
{
}

}
