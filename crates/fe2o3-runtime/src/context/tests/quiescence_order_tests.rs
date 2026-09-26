use super::*;
use crate::context::versions::completion_faults::{
    CompletionJournalFailureV1, CompletionJournalPointV1, CompletionJournalStageV1,
};
use std::sync::{Arc, Mutex};

const COUNT: usize = 16;

struct Fixture {
    context: RuntimeContextV1<MockBackend>,
    stream: RuntimeStreamIdV1,
    submissions: Vec<RuntimeSubmissionV1<AddArguments>>,
    callbacks: Arc<Mutex<Vec<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>>,
}

impl Fixture {
    fn new(journal: bool) -> Self {
        let (mut context, stream, first, kernel) = if journal {
            async_journal_tests::fixture(COUNT)
        } else {
            context_with_launch_prerequisites()
        };
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        let mut submissions = Vec::new();
        for index in 0..COUNT {
            let allocation = if index == 0 {
                first
            } else {
                context
                    .allocate(
                        context.devices()[0].id(),
                        RuntimeMemoryKindV1::DeviceLocal,
                        64,
                        16,
                    )
                    .unwrap()
            };
            let submission = context
                .launch(
                    stream,
                    &kernel,
                    &AddArguments {
                        allocation,
                        scalar: 1,
                    },
                    geometry(),
                    &[],
                )
                .unwrap();
            let id = submission.id;
            let calls = Arc::clone(&callbacks);
            context
                .on_completion(&submission, move |status| {
                    calls.lock().unwrap().push((id, status))
                })
                .unwrap();
            submissions.push(submission);
        }
        assert!(submissions.windows(2).all(|pair| pair[0].id < pair[1].id));
        Self {
            context,
            stream,
            submissions,
            callbacks,
        }
    }

    fn assert_callbacks(&self, count: usize) {
        let expected: Vec<_> = self.submissions[..count]
            .iter()
            .map(|submission| {
                (
                    submission.id,
                    RuntimeCompletionStatusV1::QuiescentWithoutResult,
                )
            })
            .collect();
        assert_eq!(*self.callbacks.lock().unwrap(), expected);
        assert_eq!(self.context.completion_callback_count, COUNT - count);
    }
}

#[test]
fn stream_quiescence_notifies_in_ascending_id_order_once() {
    for _ in 0..4 {
        for journal in [false, true] {
            for cleanup in [false, true] {
                for failure in [
                    MockCleanupFailure::None,
                    MockCleanupFailure::QuiescentStreamOnce,
                    MockCleanupFailure::RejectStreamOnce,
                ] {
                    let mut f = Fixture::new(journal);
                    f.context.backend.cleanup_failure = failure;
                    if cleanup {
                        let report = f.context.cleanup();
                        assert_eq!(report.is_complete(), failure == MockCleanupFailure::None);
                        assert_eq!(
                            report.failures().len(),
                            usize::from(failure != MockCleanupFailure::None)
                        );
                    } else {
                        let result = f.context.destroy_stream(f.stream);
                        match failure {
                            MockCleanupFailure::None => result.unwrap(),
                            MockCleanupFailure::QuiescentStreamOnce => {
                                assert!(matches!(result, Err(RuntimeErrorV1::BackendQuiescent(_))))
                            }
                            MockCleanupFailure::RejectStreamOnce => {
                                assert!(matches!(result, Err(RuntimeErrorV1::BackendRejected(_))))
                            }
                            _ => unreachable!(),
                        }
                    }
                    if failure == MockCleanupFailure::RejectStreamOnce {
                        f.assert_callbacks(0);
                        if cleanup {
                            assert!(f.context.cleanup().is_complete());
                        } else {
                            f.context.destroy_stream(f.stream).unwrap();
                        }
                    }
                    f.assert_callbacks(COUNT);
                    if !cleanup {
                        for submission in &f.submissions {
                            assert_eq!(
                                f.context.query_submission(submission).unwrap(),
                                RuntimeCompletionStatusV1::QuiescentWithoutResult
                            );
                        }
                    }
                    assert!(f.context.cleanup().is_complete());
                    f.assert_callbacks(COUNT);
                    assert!(!f.context.is_terminal());
                }
            }
        }
    }
}

#[test]
fn stream_quiescence_error_retains_the_exact_sorted_suffix() {
    for _ in 0..4 {
        for cleanup in [false, true] {
            for quiescent_failure in [false, true] {
                let mut f = Fixture::new(true);
                let middle = COUNT / 2;
                let before = f.context.cleanup_report(Vec::new()).retained;
                let writers: Vec<_> = f
                    .submissions
                    .iter()
                    .map(|submission| {
                        f.context.submissions[&submission.id]
                            .journal_writer
                            .unwrap()
                    })
                    .collect();
                f.context
                    .versions
                    .as_mut()
                    .unwrap()
                    .inject_completion_fault_for_test_v1(
                        f.submissions[middle].id,
                        CompletionJournalStageV1::Writer,
                        CompletionJournalPointV1::BeforeEffect,
                        CompletionJournalFailureV1::Error,
                    );
                if quiescent_failure {
                    f.context.backend.cleanup_failure = MockCleanupFailure::QuiescentStreamOnce;
                }
                if cleanup {
                    let report = f.context.cleanup();
                    assert!(!report.is_complete());
                    assert!(report.terminal);
                    assert_eq!(report.failures().len(), usize::from(quiescent_failure));
                    if quiescent_failure {
                        assert!(matches!(
                            report.failures()[0].failure(),
                            RuntimeBackendFailureV1::Quiescent(MockError(
                                "destroy failed after quiescence"
                            ))
                        ));
                    }
                } else {
                    let result = f.context.destroy_stream(f.stream);
                    if quiescent_failure {
                        assert!(matches!(result, Err(RuntimeErrorV1::BackendQuiescent(_))));
                    } else {
                        assert!(matches!(
                            result,
                            Err(RuntimeErrorV1::Validation(
                                RuntimeValidationErrorV1::InvalidBackendDescription
                            ))
                        ));
                    }
                }
                f.assert_callbacks(middle);
                assert!(f.context.is_terminal());
                assert!(
                    !f.context
                        .versions
                        .as_ref()
                        .unwrap()
                        .completion_fault_pending_for_test_v1()
                );
                assert_eq!(f.context.cleanup_report(Vec::new()).retained, before);
                for (index, submission) in f.submissions.iter().enumerate() {
                    let record = &f.context.submissions[&submission.id];
                    assert_eq!(record.journal_writer, Some(writers[index]));
                    assert_eq!(record.quiescent, index < middle);
                    assert_eq!(
                        record.status,
                        if index < middle {
                            RuntimeCompletionStatusV1::QuiescentWithoutResult
                        } else {
                            RuntimeCompletionStatusV1::Pending
                        }
                    );
                }
                // Unknown output is not a completed writer and cannot retire its root.
                assert_eq!(f.context.version_journal_writer_records_v1(), Some(COUNT));
                let calls = f.context.backend.cleanup_log.clone();
                assert!(!f.context.cleanup().is_complete());
                assert_eq!(f.context.backend.cleanup_log, calls);
                f.assert_callbacks(middle);
            }
        }
    }
}
