use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Ready, Published }
pub open spec fn mutated_partial_completion_phase_v1() -> PhaseV1 { PhaseV1::Ready }
pub proof fn mutated_partial_completion_may_release_continuation_v1()
    ensures mutated_partial_completion_phase_v1() == PhaseV1::Published, {}
}
