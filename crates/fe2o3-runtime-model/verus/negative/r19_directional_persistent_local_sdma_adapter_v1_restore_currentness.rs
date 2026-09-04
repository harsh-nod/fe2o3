use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Restored, Quarantined }
pub open spec fn mutated_restore_v1(child_current: bool) -> PhaseV1 { PhaseV1::Restored }
pub proof fn mutated_restore_currentness_ambiguity_quarantines_v1()
    ensures mutated_restore_v1(false) == PhaseV1::Quarantined,
{}
}
