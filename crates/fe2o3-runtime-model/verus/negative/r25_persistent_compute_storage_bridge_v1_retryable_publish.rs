use vstd::prelude::*;
verus! {
pub open spec fn prepared_phase_v1() -> nat { 1 }
pub open spec fn mutated_retryable_publish_phase_v1() -> nat { 2 }
pub proof fn mutated_retryable_publish_is_no_effect_v1()
    ensures mutated_retryable_publish_phase_v1() == prepared_phase_v1(), {}
}
