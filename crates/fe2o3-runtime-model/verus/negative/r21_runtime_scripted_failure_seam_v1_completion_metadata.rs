use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Published, ProcessTeardown }
pub open spec fn mutated_completion_mismatch_phase_v1(_matches: bool) -> PhaseV1 {
    PhaseV1::Published
}
pub proof fn mutated_completion_metadata_mismatch_tears_down_v1()
    ensures mutated_completion_mismatch_phase_v1(false) == PhaseV1::ProcessTeardown, {}
}
