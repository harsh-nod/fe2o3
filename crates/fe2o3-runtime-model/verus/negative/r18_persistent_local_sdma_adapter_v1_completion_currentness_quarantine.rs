use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Published, CompletionRestored, Quarantined }
pub open spec fn mutated_completion_v1(post_current: bool) -> PhaseV1 {
    if post_current { PhaseV1::CompletionRestored } else { PhaseV1::Published }
}
pub proof fn mutated_incomplete_completion_currentness_quarantines_v1()
    ensures mutated_completion_v1(false) == PhaseV1::Quarantined,
{}
}
