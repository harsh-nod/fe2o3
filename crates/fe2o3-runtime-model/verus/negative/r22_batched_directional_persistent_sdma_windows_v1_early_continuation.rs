use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Ready, FrontierPending }
pub open spec fn mutated_completed_window_phase_v1() -> PhaseV1 { PhaseV1::Ready }
pub proof fn mutated_continuation_may_precede_frontier_retirement_v1()
    ensures mutated_completed_window_phase_v1() == PhaseV1::FrontierPending, {}
}
