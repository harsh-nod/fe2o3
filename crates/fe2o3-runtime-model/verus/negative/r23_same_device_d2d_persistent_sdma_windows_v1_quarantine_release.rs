use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Ready, Quarantined }
pub open spec fn mutated_release_quarantine_v1() -> PhaseV1 { PhaseV1::Ready }
pub proof fn mutated_d2d_quarantine_has_no_release_route_v1()
    ensures mutated_release_quarantine_v1() == PhaseV1::Quarantined, {}
}
