use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Published, FrontierPending, Ready }
pub open spec fn mutated_complete_v1(_phase: PhaseV1) -> PhaseV1 { PhaseV1::Ready }
pub proof fn mutated_completion_waits_for_frontier_retirement_v1()
    ensures mutated_complete_v1(PhaseV1::Published) == PhaseV1::FrontierPending, {}
}
