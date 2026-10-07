use vstd::prelude::*;
verus! {
pub open spec fn detached_generation_v1() -> nat { 7 }
pub open spec fn mutated_expected_predecessor_v1() -> nat { 8 }
pub proof fn mutated_replay_requires_exact_recycled_predecessor_v1()
    ensures mutated_expected_predecessor_v1() == detached_generation_v1(), {}
}
