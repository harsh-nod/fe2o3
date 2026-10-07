// Expected negative: quarantined slots reenter the free-record pool.
use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Reserved, Retained, Quarantined, Vacant }
pub open spec fn reusable_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Vacant || phase == PhaseV1::Quarantined
}
pub proof fn mutated_quarantine_reuse_v1(phase: PhaseV1)
    requires reusable_v1(phase),
    ensures phase == PhaseV1::Vacant,
{}
}
