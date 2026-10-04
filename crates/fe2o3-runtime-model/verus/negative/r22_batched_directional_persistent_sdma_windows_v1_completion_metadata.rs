use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Published, ProcessTeardown }
pub open spec fn mutated_completion_mismatch_phase_v1() -> PhaseV1 { PhaseV1::Published }
pub proof fn mutated_completion_metadata_substitution_may_continue_v1()
    ensures mutated_completion_mismatch_phase_v1() == PhaseV1::ProcessTeardown, {}
}
