// Expected-negative R60 mutation: foreign completion leaves the lane current.
use vstd::prelude::*;

verus! {

pub struct LaneV1 { pub current: bool, pub quarantined: bool }

pub open spec fn mutated_foreign_completion_v1(lane: LaneV1) -> LaneV1 { lane }

pub proof fn mutated_foreign_completion_quarantines_lane_v1(lane: LaneV1)
    requires lane.current, !lane.quarantined,
    ensures {
        let after = mutated_foreign_completion_v1(lane);
        after.quarantined && !after.current
    },
{
}

}
