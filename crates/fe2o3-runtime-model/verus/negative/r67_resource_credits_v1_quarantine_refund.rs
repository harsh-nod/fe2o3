// Expected negative: quarantining retained custody vacates its charged record.
use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Reserved, Retained, Quarantined, Vacant }
pub open spec fn quarantine_v1(phase: PhaseV1) -> PhaseV1 {
    if phase == PhaseV1::Retained { PhaseV1::Vacant } else { phase }
}
pub proof fn mutated_quarantine_refund_v1(phase: PhaseV1)
    requires phase == PhaseV1::Retained,
    ensures quarantine_v1(phase) != PhaseV1::Vacant,
{}
}
