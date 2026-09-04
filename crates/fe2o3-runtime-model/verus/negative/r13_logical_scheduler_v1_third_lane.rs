use vstd::prelude::*;

verus! {

pub open spec fn physical_lane_count_v1() -> nat { 2 }

pub proof fn mutated_third_physical_lane_is_supported_v1()
    ensures physical_lane_count_v1() >= 3,
{
}

}
