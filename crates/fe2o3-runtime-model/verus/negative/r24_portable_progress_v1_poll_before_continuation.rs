use vstd::prelude::*;
verus! {
pub enum WindowObservationV1 { Submitted, Polled }
pub open spec fn mutated_first_window_observation_v1() -> WindowObservationV1 {
    WindowObservationV1::Submitted
}
pub proof fn mutated_continuation_requires_prior_poll_v1()
    ensures mutated_first_window_observation_v1() == WindowObservationV1::Polled, {}
}
