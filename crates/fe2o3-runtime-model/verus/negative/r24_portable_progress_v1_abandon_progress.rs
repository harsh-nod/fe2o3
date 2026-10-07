use vstd::prelude::*;
verus! {
pub open spec fn mutated_phase_before_abandon_v1() -> nat { 63 }
pub open spec fn mutated_phase_after_abandon_v1() -> nat { 2 }
pub proof fn mutated_abandon_is_observation_only_v1()
    ensures mutated_phase_before_abandon_v1() == mutated_phase_after_abandon_v1(), {}
}
