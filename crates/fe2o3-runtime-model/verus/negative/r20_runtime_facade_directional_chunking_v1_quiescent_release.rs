use vstd::prelude::*;
verus! {
pub enum PhaseV1 { QuiescentWithoutResult, Published }
pub open spec fn mutated_flush_quiescent_v1(_phase: PhaseV1) -> PhaseV1 { PhaseV1::Published }
pub proof fn mutated_quiescent_target_is_not_resumable_v1()
    ensures mutated_flush_quiescent_v1(PhaseV1::QuiescentWithoutResult)
        == PhaseV1::QuiescentWithoutResult, {}
}
