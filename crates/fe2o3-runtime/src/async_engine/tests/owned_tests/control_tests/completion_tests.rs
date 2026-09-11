use super::*;
use crate::async_engine::generated_operation::completion_contract::{
    CompletionClassV1 as Class, classify_completion_v1 as classify,
};
use crate::kfd_backend::counted_allocations_for_test_v1 as counted;
use crate::{
    RuntimeBackendProtocolErrorV1 as Protocol, RuntimeBackendResourceKindV1 as Kind,
    RuntimeCompletionFailureV1 as Failure, RuntimeCompletionStatusV1 as Status,
    RuntimeErrorV1 as Error, RuntimeValidationErrorV1 as Validation,
};

#[test]
fn co1_classifier_covers_all_observation_families_without_allocation() {
    let cases: [(Result<Status, Error<()>>, Class); 13] = [
        (Ok(Status::Pending), Class::Pending),
        (Ok(Status::Succeeded), Class::SuccessCandidate),
        (
            Ok(Status::Failed(Failure::BackendCode(-17))),
            Class::Failed(Failure::BackendCode(-17)),
        ),
        (
            Ok(Status::Failed(Failure::Cancelled)),
            Class::Failed(Failure::Cancelled),
        ),
        (
            Ok(Status::QuiescentWithoutResult),
            Class::QuiescentWithoutResult,
        ),
        (Err(Error::BackendRejected(())), Class::Rejected),
        (Err(Error::BackendQuiescent(())), Class::QuiescentError),
        (Err(Error::BackendTerminal(())), Class::Terminal),
        (
            Err(Error::BackendProtocol(Protocol::ZeroHandle(
                Kind::Submission,
            ))),
            Class::Terminal,
        ),
        (
            Err(Error::BackendProtocol(Protocol::DuplicateHandle(
                Kind::Submission,
            ))),
            Class::Terminal,
        ),
        (
            Err(Error::Validation(Validation::ContextTerminal)),
            Class::Terminal,
        ),
        (
            Err(Error::Validation(Validation::UnknownSubmission)),
            Class::ObservationError,
        ),
        (
            Err(Error::Validation(Validation::ContextReserved)),
            Class::ObservationError,
        ),
    ];
    let expected = cases.each_ref().map(|(_, class)| *class);
    let (actual, allocations) = counted(|| {
        cases
            .each_ref()
            .map(|(observation, _)| classify(std::hint::black_box(observation)))
    });
    assert_eq!(actual, expected);
    assert_eq!(allocations, 0);
    for code in [i64::MIN, -3, -2, 0, i64::MAX] {
        let observation: Result<Status, Error<()>> = Ok(Status::Failed(Failure::BackendCode(code)));
        assert_eq!(
            classify(&observation),
            Class::Failed(Failure::BackendCode(code))
        );
    }
}

#[test]
fn co1_classifier_borrows_non_clone_errors_without_early_drop() {
    struct Probe<'a>(&'a Cell<usize>);
    impl Drop for Probe<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0);
    for mode in 0..3 {
        let error = Probe(&drops);
        let observation: Result<Status, Error<Probe<'_>>> = Err(match mode {
            0 => Error::BackendRejected(error),
            1 => Error::BackendQuiescent(error),
            _ => Error::BackendTerminal(error),
        });
        let expected = [Class::Rejected, Class::QuiescentError, Class::Terminal][mode];
        let (actual, allocations) = counted(|| {
            [
                classify(std::hint::black_box(&observation)),
                classify(std::hint::black_box(&observation)),
            ]
        });
        assert_eq!(actual, [expected; 2]);
        assert_eq!(allocations, 0);
        assert_eq!(drops.get(), mode);
        drop(observation);
        assert_eq!(drops.get(), mode + 1);
    }
}

#[test]
fn co1_existing_reply_gate_is_sticky_and_preserves_credit() {
    let budget = reply_budget::ReplyBudgetV1::new(1);
    let (mut reply, mut future) = owned::Reply::budgeted_pair(&budget).unwrap();
    reply.complete(Ok(17_u8));
    reply.complete(Ok(29));
    assert_eq!(poll(&mut future, Waker::noop()), Poll::Ready(Ok(17)));
    assert_eq!(budget.used(), 1);
    drop(future);
    assert_eq!(budget.used(), 1);
    drop(reply);
    assert_eq!(budget.used(), 0);
}

#[test]
fn co1_ordinary_rejections_keep_one_issue_and_exact_last_error() {
    let mut h = Harness::new();
    let mut future = h.launch();
    let control = future.control();
    let mut driver = h.receive();
    assert!(poll(&mut future, Waker::noop()).is_pending());
    assert!(!driver.advance(&mut h.context));
    let issued = {
        let state = h.state.lock().unwrap();
        assert_eq!(state.issues.len(), 1);
        assert_eq!(state.poll_calls, 0);
        (state.issues[0].0, state.issues[0].1)
    };
    for (index, message) in ["first", "second"].into_iter().enumerate() {
        h.state
            .lock()
            .unwrap()
            .poll_failures
            .push_back(RuntimeBackendFailureV1::Rejected(MockError(message)));
        assert!(!driver.advance(&mut h.context));
        assert!(poll(&mut future, Waker::noop()).is_pending());
        assert_eq!(h.state.lock().unwrap().poll_calls, index + 1);
        assert_eq!(control.phase(), Phase::Observing);
    }
    assert!(!driver.advance(&mut h.context));
    assert!(poll(&mut future, Waker::noop()).is_pending());
    assert_eq!(h.state.lock().unwrap().poll_calls, 3);
    h.succeed();
    assert!(driver.advance(&mut h.context));
    let Poll::Ready(Ok(result)) = poll(&mut future, Waker::noop()) else {
        panic!("one successful observation must resolve the existing reply");
    };
    assert!(matches!(result.observation, Ok(Status::Succeeded)));
    assert_eq!(result.rejected_observations, 2);
    assert_eq!(result.last_rejected_observation, Some(MockError("second")));
    assert_eq!(
        h.context
            .query_submission(result.submission.as_ref().unwrap()),
        Ok(Status::Succeeded)
    );
    assert_eq!(control.phase(), Phase::ObservationFinished);
    {
        let state = h.state.lock().unwrap();
        assert_eq!(state.poll_calls, 4);
        assert_eq!(state.issues.len(), 1);
        assert_eq!((state.issues[0].0, state.issues[0].1), issued);
        assert_eq!(state.release_calls, 0);
    }
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn co1_ordinary_failure_and_quiescent_error_preserve_distinct_raw_results() {
    for quiescent in [false, true] {
        let mut h = Harness::new();
        let mut future = h.launch();
        let mut driver = h.receive();
        assert!(!driver.advance(&mut h.context));
        {
            let mut state = h.state.lock().unwrap();
            if quiescent {
                state
                    .poll_failures
                    .push_back(RuntimeBackendFailureV1::Quiescent(MockError(
                        "retired without result",
                    )));
            } else {
                for status in state.statuses.values_mut() {
                    *status = BackendPollV1::Failed { code: 17 };
                }
            }
        }
        assert!(driver.advance(&mut h.context));
        let Poll::Ready(Ok(result)) = poll(&mut future, Waker::noop()) else {
            panic!("raw terminal observation must be reported");
        };
        let expected = if quiescent {
            assert!(matches!(
                result.observation,
                Err(Error::BackendQuiescent(MockError("retired without result")))
            ));
            Status::QuiescentWithoutResult
        } else {
            assert!(matches!(
                result.observation,
                Ok(Status::Failed(Failure::BackendCode(17)))
            ));
            Status::Failed(Failure::BackendCode(17))
        };
        assert_eq!(
            h.context
                .query_submission(result.submission.as_ref().unwrap()),
            Ok(expected)
        );
        assert_eq!(result.rejected_observations, 0);
        assert_eq!(result.last_rejected_observation, None);
        {
            let state = h.state.lock().unwrap();
            assert_eq!(state.poll_calls, 1);
            assert_eq!(state.issues.len(), 1);
            assert_eq!(state.release_calls, 0);
        }
        assert!(!h.context.is_terminal());
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn co1_terminal_raw_reply_does_not_retire_registry_or_context_custody() {
    let mut h = Harness::new();
    let mut future = h.launch();
    let mut registry = operation::OperationRegistryV1::new(1, true);
    registry.insert(h.receive());
    operation::advance_operations_v1(
        &mut h.context,
        &mut registry,
        1,
        1,
        flush_stream_v1::<MockBackend>,
    );
    assert_eq!(registry.active_len(), 1);
    assert!(poll(&mut future, Waker::noop()).is_pending());
    let flushes = h.state.lock().unwrap().flush_calls.len();
    h.state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Terminal(MockError("uncertain")));
    operation::advance_operations_v1(
        &mut h.context,
        &mut registry,
        1,
        1,
        flush_stream_v1::<MockBackend>,
    );
    let Poll::Ready(Ok(result)) = poll(&mut future, Waker::noop()) else {
        panic!("terminal error must reach the existing reply");
    };
    assert!(matches!(
        result.observation,
        Err(Error::BackendTerminal(MockError("uncertain")))
    ));
    assert!(result.submission.is_some());
    assert!(h.context.is_terminal());
    assert_eq!(registry.active_len(), 1);
    assert_eq!(registry.len(), 1);
    assert!(!registry.stop_observations());
    assert_eq!(registry.active_len(), 1);
    let cleanup = h.context.cleanup();
    assert!(cleanup.is_terminal());
    assert_eq!(cleanup.retained().submissions, 1);
    let state = h.state.lock().unwrap();
    assert_eq!(state.issues.len(), 1);
    assert_eq!(state.poll_calls, 1);
    assert_eq!(state.flush_calls.len(), flushes);
    assert_eq!(state.release_calls, 0);
}
