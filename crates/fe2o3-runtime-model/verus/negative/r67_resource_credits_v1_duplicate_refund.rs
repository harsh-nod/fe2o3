// Expected negative: a previously vacant record can authorize another refund.
use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Reserved, Retained, Quarantined, Vacant }
pub open spec fn release_allowed_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Retained || phase == PhaseV1::Vacant
}
pub proof fn mutated_duplicate_refund_v1(phase: PhaseV1)
    requires release_allowed_v1(phase),
    ensures phase == PhaseV1::Retained,
{}
}
