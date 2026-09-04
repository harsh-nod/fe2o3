use vstd::prelude::*;
verus! {
pub open spec fn mutated_retryable_poll_observing_v1() -> bool { true }
pub proof fn mutated_retryable_poll_retires_progress_registration_v1()
    ensures !mutated_retryable_poll_observing_v1(), {}
}
