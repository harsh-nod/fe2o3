// Expected-negative R38 mutation: timeout restores the active execution but
// loses its exact lane/submission index.
use vstd::prelude::*;

verus! {
pub struct StateV1 { pub active_lane: Option<nat>, pub lane_submission: Option<nat> }

pub open spec fn mutated_timeout_v1(lane: nat, submission: nat) -> StateV1 {
    StateV1 { active_lane: Some(lane), lane_submission: None }
}

pub proof fn mutated_timeout_restores_exact_lane_index_v1(lane: nat, submission: nat)
    ensures
        mutated_timeout_v1(lane, submission).active_lane == Some(lane),
        mutated_timeout_v1(lane, submission).lane_submission == Some(submission),
{}
}
