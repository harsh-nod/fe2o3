use vstd::prelude::*;
verus! {
pub open spec fn mutated_first_window_was_polled_v1() -> bool { false }
pub proof fn mutated_continuation_requires_prior_poll_v1()
    ensures mutated_first_window_was_polled_v1(), {}
}
