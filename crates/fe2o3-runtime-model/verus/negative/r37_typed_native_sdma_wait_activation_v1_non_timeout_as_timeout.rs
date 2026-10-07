// Expected-negative R37 mutation: a matching non-timeout retryable result is
// incorrectly accepted as a recoverable timeout.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum ObservationV1 { ExactTimeout, NonTimeoutRetryable }
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Pending, Terminal }

pub open spec fn mutated_classify_v1(observation: ObservationV1) -> OutcomeV1 {
    match observation {
        ObservationV1::ExactTimeout => OutcomeV1::Pending,
        ObservationV1::NonTimeoutRetryable => OutcomeV1::Pending,
    }
}

pub proof fn mutated_non_timeout_retryable_is_terminal_v1()
    ensures mutated_classify_v1(ObservationV1::NonTimeoutRetryable) == OutcomeV1::Terminal,
{}
}
