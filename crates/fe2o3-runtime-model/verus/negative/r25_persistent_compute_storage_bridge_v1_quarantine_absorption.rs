use vstd::prelude::*;
verus! {
pub open spec fn quarantine_phase_v1() -> nat { 6 }
pub open spec fn mutated_quarantine_transition_v1() -> nat { 5 }
pub proof fn mutated_quarantine_is_absorbing_v1()
    ensures mutated_quarantine_transition_v1() == quarantine_phase_v1(), {}
}
