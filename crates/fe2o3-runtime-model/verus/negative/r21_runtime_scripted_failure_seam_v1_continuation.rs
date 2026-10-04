use vstd::prelude::*;
verus! {
pub enum PhaseV1 { TerminalObserved, RecyclePending, Ready }
pub open spec fn mutated_post_retirement_phase_v1(_phase: PhaseV1) -> PhaseV1 {
    PhaseV1::Ready
}
pub proof fn mutated_continuation_waits_for_recycle_v1()
    ensures mutated_post_retirement_phase_v1(PhaseV1::TerminalObserved)
        == PhaseV1::RecyclePending, {}
}
