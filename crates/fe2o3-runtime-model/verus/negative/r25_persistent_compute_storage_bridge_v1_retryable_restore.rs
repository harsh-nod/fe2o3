use vstd::prelude::*;
verus! {
pub open spec fn completed_phase_v1() -> nat { 3 }
pub open spec fn mutated_retryable_restore_phase_v1() -> nat { 4 }
pub proof fn mutated_retryable_restore_is_no_effect_v1()
    ensures mutated_retryable_restore_phase_v1() == completed_phase_v1(), {}
}
