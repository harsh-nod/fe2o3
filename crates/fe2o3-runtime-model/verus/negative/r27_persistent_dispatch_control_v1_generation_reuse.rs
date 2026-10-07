use vstd::prelude::*;
verus! {
pub open spec fn recycled_predecessor_v1() -> nat { 11 }
pub open spec fn mutated_replay_generation_v1() -> nat { 11 }
pub proof fn mutated_replay_generation_strictly_advances_v1()
    ensures mutated_replay_generation_v1() > recycled_predecessor_v1(), {}
}
