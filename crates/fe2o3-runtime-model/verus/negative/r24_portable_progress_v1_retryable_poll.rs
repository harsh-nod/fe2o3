use vstd::prelude::*;
verus! {
pub open spec fn mutated_poll_custody_before_v1() -> bool { true }
pub open spec fn mutated_poll_custody_after_v1() -> bool { false }
pub proof fn mutated_retryable_poll_preserves_custody_v1()
    ensures mutated_poll_custody_before_v1() == mutated_poll_custody_after_v1(), {}
}
