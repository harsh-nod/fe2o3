//! Descriptive host decisions, not GPU completion or publication authority.
//!
//! The separately authenticated Verus model is checked against this finite
//! transition table by review. Tests exhaust the Rust table against an independent
//! numeric oracle; they do not execute Verus. Atomic linearization is contracted.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum R62OperationPhaseV1 {
    Queued,
    CancelledBeforeSubmission,
    SubmissionStarted,
    Observing,
    ObservationFinished,
    StoppedBeforeSubmission,
    StoppedAfterSubmission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R62OperationActionV1 {
    Cancel,
    Start,
    AcceptSubmission,
    FinishObservation,
    Stop,
    ObserveTimeout,
    DropObserver,
}

pub const fn r62_operation_transition_v1(
    phase: R62OperationPhaseV1,
    action: R62OperationActionV1,
) -> R62OperationPhaseV1 {
    use R62OperationActionV1 as A;
    use R62OperationPhaseV1 as P;
    match (phase, action) {
        (P::Queued, A::Cancel) => P::CancelledBeforeSubmission,
        (P::Queued, A::Start) => P::SubmissionStarted,
        (P::SubmissionStarted, A::AcceptSubmission) => P::Observing,
        (P::SubmissionStarted | P::Observing, A::FinishObservation) => P::ObservationFinished,
        (P::Queued, A::Stop) => P::StoppedBeforeSubmission,
        (P::SubmissionStarted | P::Observing, A::Stop) => P::StoppedAfterSubmission,
        _ => phase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use R62OperationActionV1 as A;
    use R62OperationPhaseV1 as P;

    const PHASES: [P; 7] = [
        P::Queued,
        P::CancelledBeforeSubmission,
        P::SubmissionStarted,
        P::Observing,
        P::ObservationFinished,
        P::StoppedBeforeSubmission,
        P::StoppedAfterSubmission,
    ];
    const ACTIONS: [A; 7] = [
        A::Cancel,
        A::Start,
        A::AcceptSubmission,
        A::FinishObservation,
        A::Stop,
        A::ObserveTimeout,
        A::DropObserver,
    ];

    #[test]
    fn exhaustive_table_matches_reviewed_numeric_oracle() {
        // Independently encoded Rust oracle; Verus correspondence is reviewed.
        let expected = [
            [1, 2, 0, 0, 5, 0, 0],
            [1, 1, 1, 1, 1, 1, 1],
            [2, 2, 3, 4, 6, 2, 2],
            [3, 3, 3, 4, 6, 3, 3],
            [4, 4, 4, 4, 4, 4, 4],
            [5, 5, 5, 5, 5, 5, 5],
            [6, 6, 6, 6, 6, 6, 6],
        ];
        for (row, phase) in PHASES.into_iter().enumerate() {
            for (column, action) in ACTIONS.into_iter().enumerate() {
                assert_eq!(
                    r62_operation_transition_v1(phase, action) as u8,
                    expected[row][column]
                );
            }
        }
    }

    #[test]
    fn all_short_traces_preserve_cancellation_and_start_custody() {
        fn visit(phase: P, starts: usize, cancelled: bool, depth: usize) {
            assert!(starts <= 1);
            assert!(!cancelled || starts == 0);
            if depth == 0 {
                return;
            }
            for action in ACTIONS {
                let next = r62_operation_transition_v1(phase, action);
                assert!(phase == P::Queued || next != P::Queued);
                let starts =
                    starts + usize::from(phase == P::Queued && next == P::SubmissionStarted);
                visit(
                    next,
                    starts,
                    cancelled || next == P::CancelledBeforeSubmission,
                    depth - 1,
                );
            }
        }
        visit(P::Queued, 0, false, 6);
    }
}
