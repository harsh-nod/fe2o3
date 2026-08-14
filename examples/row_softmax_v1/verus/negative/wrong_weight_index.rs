use vstd::prelude::*;

verus! {

pub open spec fn row_elements_v1() -> nat { 64 }

pub proof fn mutated_lane_zero_weight_matches_every_lane_v1(
    weights: Seq<int>,
    lane: nat,
)
    requires
        weights.len() == row_elements_v1(),
        lane < row_elements_v1(),
    ensures weights[0] == weights[lane as int],
{
}

} // verus!
