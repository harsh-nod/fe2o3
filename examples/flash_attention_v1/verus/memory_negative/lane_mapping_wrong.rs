use vstd::prelude::*;
verus! {
pub proof fn mutated_lane_mapping_is_bounded_v1(lane: nat)
    requires lane < 64,
    ensures lane * 2 + 2 < 128,
{
}
}
