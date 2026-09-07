// Expected-negative R45 mutation: cancelled target custody remains live.
use vstd::prelude::*;
verus! {
pub open spec fn target_live_after_cancel_v1() -> bool { true }
pub proof fn mutated_cancelled_target_cannot_replay_v1()
    ensures !target_live_after_cancel_v1(),
{}
}
