use vstd::prelude::*;
verus! {
pub open spec fn quarantined_phase_v1() -> nat { 6 }
pub open spec fn mutated_ambiguous_publish_phase_v1() -> nat { 1 }
pub proof fn mutated_ambiguous_publish_quarantines_v1()
    ensures mutated_ambiguous_publish_phase_v1() == quarantined_phase_v1(), {}
}
