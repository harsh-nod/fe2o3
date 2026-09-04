use vstd::prelude::*;
verus! {
pub open spec fn mutated_retryable_flush_observing_before_v1() -> bool { true }
pub open spec fn mutated_retryable_flush_observing_after_v1() -> bool { false }
pub proof fn mutated_retryable_flush_preserves_registration_v1()
    ensures mutated_retryable_flush_observing_before_v1()
        == mutated_retryable_flush_observing_after_v1(), {}
}
