use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Settled, Quarantined, Released }
pub open spec fn mutated_release_v1(phase: PhaseV1) -> PhaseV1 {
    if phase == PhaseV1::Settled || phase == PhaseV1::Quarantined {
        PhaseV1::Released
    } else {
        phase
    }
}
pub proof fn mutated_permanent_quarantine_blocks_release_v1()
    ensures mutated_release_v1(PhaseV1::Quarantined) == PhaseV1::Quarantined,
{}
}
