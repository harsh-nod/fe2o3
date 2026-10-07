// Expected negative: pre-issue cancellation refunds retained native custody.
use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Reserved, Retained, Quarantined, Vacant }
pub open spec fn unissued_cancel_allowed_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Reserved || phase == PhaseV1::Retained
}
pub proof fn mutated_premature_refund_v1(phase: PhaseV1)
    requires unissued_cancel_allowed_v1(phase),
    ensures phase == PhaseV1::Reserved,
{}
}
