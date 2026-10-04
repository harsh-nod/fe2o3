use vstd::prelude::*;

verus! {

pub open spec fn mutated_inclusive_prefix_count_v1(lane: nat) -> nat {
    lane
}

pub proof fn mutated_inclusive_scan_includes_current_lane_v1(lane: nat)
    requires lane < 64,
    ensures mutated_inclusive_prefix_count_v1(lane) == lane + 1,
{
}

} // verus!
