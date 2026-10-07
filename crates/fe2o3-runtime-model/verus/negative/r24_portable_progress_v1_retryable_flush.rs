use vstd::prelude::*;
verus! {
pub enum ObserverRegistrationV1 { Registered, Removed }
pub open spec fn mutated_retryable_flush_observing_before_v1() -> ObserverRegistrationV1 {
    ObserverRegistrationV1::Registered
}
pub open spec fn mutated_retryable_flush_observing_after_v1() -> ObserverRegistrationV1 {
    ObserverRegistrationV1::Removed
}
pub proof fn mutated_retryable_flush_preserves_registration_v1()
    ensures mutated_retryable_flush_observing_before_v1()
        == mutated_retryable_flush_observing_after_v1(), {}
}
