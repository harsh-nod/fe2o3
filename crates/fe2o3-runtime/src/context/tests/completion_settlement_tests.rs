use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Call {
    stage: usize,
    submission: RuntimeSubmissionIdV1,
    outcome: Option<u8>,
    status: Option<RuntimeCompletionStatusV1>,
}

fn outcome_tag(outcome: SubmissionWriterOutcomeV1) -> u8 {
    match outcome {
        SubmissionWriterOutcomeV1::Success => 1,
        SubmissionWriterOutcomeV1::NoEffect => 2,
        SubmissionWriterOutcomeV1::Unknown => 3,
    }
}

struct Probe {
    fail_at: Option<usize>,
    panic: bool,
    calls: Vec<Call>,
    completed: Vec<usize>,
    returned: RuntimeCompletionStatusV1,
}

impl Probe {
    fn record(&mut self, call: Call) -> Result<(), (usize, u64)> {
        let stage = call.stage;
        self.calls.push(call);
        if self.fail_at == Some(stage) {
            assert!(!self.panic, "settlement stage {stage}");
            return Err((stage, u64::MAX));
        }
        self.completed.push(stage);
        Ok(())
    }

    fn release_submission_inputs_v1(
        &mut self,
        submission: RuntimeSubmissionIdV1,
    ) -> Result<(), (usize, u64)> {
        self.record(Call {
            stage: 0,
            submission,
            outcome: None,
            status: None,
        })
    }

    fn settle_submission_writer_v1(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        outcome: SubmissionWriterOutcomeV1,
    ) -> Result<(), (usize, u64)> {
        self.record(Call {
            stage: 1,
            submission,
            outcome: Some(outcome_tag(outcome)),
            status: None,
        })
    }

    fn release_operation_dependencies_v1(
        &mut self,
        submission: RuntimeSubmissionIdV1,
    ) -> Result<(), (usize, u64)> {
        self.record(Call {
            stage: 2,
            submission,
            outcome: None,
            status: None,
        })
    }

    fn publish_submission_status_v1(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        status: RuntimeCompletionStatusV1,
    ) -> Result<RuntimeCompletionStatusV1, (usize, u64)> {
        self.record(Call {
            stage: 3,
            submission,
            outcome: None,
            status: Some(status),
        })?;
        Ok(self.returned)
    }

    #[allow(clippy::question_mark)]
    fn settle(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        status: RuntimeCompletionStatusV1,
        outcome: SubmissionWriterOutcomeV1,
    ) -> Result<RuntimeCompletionStatusV1, (usize, u64)> {
        completion_settlement_execution_body!(
            completion_settlement_rust_expr,
            self,
            submission,
            status,
            outcome
        )
    }
}

#[test]
fn settlement_body_preserves_arguments_result_and_failed_prefixes() {
    let submission = RuntimeSubmissionIdV1::new(17, 83);
    for outcome in [
        SubmissionWriterOutcomeV1::Success,
        SubmissionWriterOutcomeV1::NoEffect,
        SubmissionWriterOutcomeV1::Unknown,
    ] {
        for status in [
            RuntimeCompletionStatusV1::Pending,
            RuntimeCompletionStatusV1::Succeeded,
            RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled),
            RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(i64::MIN)),
            RuntimeCompletionStatusV1::QuiescentWithoutResult,
        ] {
            let expected: Vec<_> = (0..4)
                .map(|stage| Call {
                    stage,
                    submission,
                    outcome: (stage == 1).then_some(outcome_tag(outcome)),
                    status: (stage == 3).then_some(status),
                })
                .collect();
            for fail_at in [None, Some(0), Some(1), Some(2), Some(3)] {
                let mut probe = Probe {
                    fail_at,
                    panic: false,
                    calls: Vec::new(),
                    completed: Vec::new(),
                    returned: RuntimeCompletionStatusV1::Failed(
                        RuntimeCompletionFailureV1::BackendCode(91),
                    ),
                };
                assert_eq!(
                    probe.settle(submission, status, outcome),
                    fail_at.map_or(Ok(probe.returned), |stage| Err((stage, u64::MAX)))
                );
                assert_eq!(
                    probe.calls,
                    expected[..fail_at.map_or(4, |stage| stage + 1)]
                );
                assert_eq!(
                    probe.completed,
                    (0..fail_at.unwrap_or(4)).collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn settlement_body_does_not_continue_after_unwind() {
    for stage in 0..4 {
        let mut probe = Probe {
            fail_at: Some(stage),
            panic: true,
            calls: Vec::new(),
            completed: Vec::new(),
            returned: RuntimeCompletionStatusV1::Succeeded,
        };
        assert!(
            catch_unwind(AssertUnwindSafe(|| probe.settle(
                RuntimeSubmissionIdV1::new(17, 83),
                RuntimeCompletionStatusV1::Succeeded,
                SubmissionWriterOutcomeV1::Success
            )))
            .is_err()
        );
        assert_eq!(
            probe
                .calls
                .iter()
                .map(|call| call.stage)
                .collect::<Vec<_>>(),
            (0..=stage).collect::<Vec<_>>()
        );
        assert_eq!(probe.completed, (0..stage).collect::<Vec<_>>());
    }
}
