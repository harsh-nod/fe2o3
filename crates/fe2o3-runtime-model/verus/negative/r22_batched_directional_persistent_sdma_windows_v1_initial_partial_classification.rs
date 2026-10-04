use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Completed, QuiescentWithoutResult }
pub open spec fn mutated_postpublication_recovery_v1() -> PhaseV1 { PhaseV1::Completed }
pub proof fn mutated_initial_postpublication_recovery_may_be_conclusive_v1()
    ensures mutated_postpublication_recovery_v1() == PhaseV1::QuiescentWithoutResult, {}
}
