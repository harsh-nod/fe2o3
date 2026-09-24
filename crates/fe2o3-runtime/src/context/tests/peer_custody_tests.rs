use super::async_journal_tests::region;
use super::*;

type Context = RuntimeContextV1<MockBackend>;
type Submission = RuntimeSubmissionV1<RuntimePeerCopyV1>;

struct Fixture {
    context: Context,
    stream: RuntimeStreamIdV1,
    source: RuntimeAllocationIdV1,
    destinations: Vec<RuntimeAllocationIdV1>,
}

impl Fixture {
    fn new(journal: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, 16, 8).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let source_device = context.devices()[0].id();
        let device = context.devices()[1].id();
        let stream = context.create_stream(device).unwrap();
        let source = context
            .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        context.write_allocation(source, 0, &[0x51; 64]).unwrap();
        let destinations = (0..4)
            .map(|_| {
                context
                    .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                    .unwrap()
            })
            .collect();
        Self {
            context,
            stream,
            source,
            destinations,
        }
    }

    fn copy(
        &mut self,
        index: usize,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<Submission, RuntimeErrorV1<MockError>> {
        self.context.peer_copy(
            self.stream,
            region(self.source, RuntimeAccessV1::Read, 8),
            region(self.destinations[index], RuntimeAccessV1::Write, 24),
            dependencies,
        )
    }

    fn retains(&self, submission: &Submission) -> usize {
        self.context.submissions[&submission.id].dependency_retains
    }
}

#[test]
fn fanout_retains_producer_after_public_event_release_until_last_consumer() {
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let mut producer = f.copy(0, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        let mut first = f.copy(1, &[event]).unwrap();
        let mut second = f.copy(2, &[event]).unwrap();
        assert_eq!(f.retains(&producer), 2);
        f.context.release_event(event).unwrap();
        f.context.wait(&mut producer, Duration::ZERO).unwrap();
        for remaining in [2, 1] {
            assert_eq!(f.retains(&producer), remaining);
            let failure = f.context.release_submission(producer).unwrap_err();
            assert!(matches!(
                failure.error(),
                RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::SubmissionRetainedByDependency
                )
            ));
            (producer, _) = failure.into_parts();
            let consumer = if remaining == 2 {
                &mut first
            } else {
                &mut second
            };
            f.context.wait(consumer, Duration::ZERO).unwrap();
            assert!(!f.context.scalar_peer_copies[&consumer.id].dependencies_held);
            f.context.poll(consumer).unwrap();
        }
        assert_eq!(f.retains(&producer), 0);
        f.context.release_submission(producer).unwrap();
        assert!(f.context.scalar_peer_copies.contains_key(&first.id));
        f.context.release_submission(first).unwrap();
        f.context.release_submission(second).unwrap();
        assert!(f.context.scalar_peer_copies.is_empty());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn full_event_roster_keeps_aliases_and_original_order_with_balanced_counts() {
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let mut first = f.copy(0, &[]).unwrap();
        let mut second = f.copy(1, &[]).unwrap();
        let a = f.context.record_event(&first).unwrap();
        let b = f.context.record_event(&first).unwrap();
        let c = f.context.record_event(&second).unwrap();
        let events = [c, b, a];
        let mut consumer = f.copy(2, &events).unwrap();
        let root = &f.context.scalar_peer_copies[&consumer.id];
        assert_eq!(
            root.backend_stream,
            f.context.streams[&f.stream].backend_stream
        );
        assert_eq!(
            root.source.region,
            region(f.source, RuntimeAccessV1::Read, 8)
        );
        assert_eq!(
            root.destination.region,
            region(f.destinations[2], RuntimeAccessV1::Write, 24)
        );
        assert_eq!(root.dependencies.len(), 3);
        for dependency in &root.dependencies {
            assert_eq!(dependency.event, events[dependency.ordinal]);
            assert_eq!(
                dependency.backend_event,
                f.context.events[&dependency.event].backend_event
            );
        }
        assert_eq!((f.retains(&first), f.retains(&second)), (2, 1));
        for event in events {
            f.context.release_event(event).unwrap();
        }
        f.context.wait(&mut consumer, Duration::ZERO).unwrap();
        assert_eq!((f.retains(&first), f.retains(&second)), (0, 0));
        // Consumer success does not manufacture producer completion.
        assert_eq!(
            f.context.query_submission(&first).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(
            f.context.query_submission(&second).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        f.context.wait(&mut first, Duration::ZERO).unwrap();
        f.context.wait(&mut second, Duration::ZERO).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn initial_backend_failures_preserve_error_and_exact_dependency_custody() {
    for journal in [false, true] {
        for failure in [
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
            MockMemoryFailure::Terminal,
            MockMemoryFailure::Panic,
        ] {
            let mut f = Fixture::new(journal);
            let producer = f.copy(0, &[]).unwrap();
            let event = f.context.record_event(&producer).unwrap();
            let attempted =
                RuntimeSubmissionIdV1::new(f.context.context_generation, f.context.next_identity);
            f.context.backend.copy_failure = failure;
            let result = catch_unwind(AssertUnwindSafe(|| f.copy(1, &[event])));
            match failure {
                MockMemoryFailure::Rejected => assert!(matches!(
                    result,
                    Ok(Err(RuntimeErrorV1::BackendRejected(_)))
                )),
                MockMemoryFailure::Quiescent => assert!(matches!(
                    result,
                    Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))
                )),
                MockMemoryFailure::Terminal => assert!(matches!(
                    result,
                    Ok(Err(RuntimeErrorV1::BackendTerminal(_)))
                )),
                MockMemoryFailure::Panic => assert!(result.is_err()),
                MockMemoryFailure::None => unreachable!(),
            }
            let retained = matches!(
                failure,
                MockMemoryFailure::Terminal | MockMemoryFailure::Panic
            );
            assert_eq!(f.context.is_terminal(), retained);
            assert_eq!(f.retains(&producer), usize::from(retained));
            assert_eq!(
                f.context.scalar_peer_copies.contains_key(&attempted),
                retained
            );
            if retained {
                let root = &f.context.scalar_peer_copies[&attempted];
                assert!(root.backend_submission.is_none());
                assert!(root.dependencies_held);
                let before = f.context.backend.cleanup_log.clone();
                let report = f.context.cleanup();
                assert_eq!(report.scalar_peer_copy_records_v1(), 2);
                assert!(!report.is_complete());
                assert_eq!(f.context.backend.cleanup_log, before);
            } else {
                assert!(f.context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn invalid_returned_handles_keep_admitted_roots_and_producer_retains() {
    for journal in [false, true] {
        for duplicate in [false, true] {
            let mut f = Fixture::new(journal);
            let producer = f.copy(0, &[]).unwrap();
            let event = f.context.record_event(&producer).unwrap();
            let attempted =
                RuntimeSubmissionIdV1::new(f.context.context_generation, f.context.next_identity);
            let handle = if duplicate {
                producer.backend_submission
            } else {
                0
            };
            f.context.backend.handle_override = Some((MockHandleKind::Submission, handle));
            assert!(f.copy(1, &[event]).is_err());
            assert!(f.context.is_terminal());
            assert_eq!(f.retains(&producer), 1);
            assert_eq!(
                f.context.scalar_peer_copies[&attempted].backend_submission,
                Some(handle)
            );
            assert!(f.context.submissions[&attempted].scalar_peer_copy);
            assert!(!f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn cancellation_and_event_completion_discharge_each_roster_once() {
    for journal in [false, true] {
        for cancel in [false, true] {
            let mut f = Fixture::new(journal);
            let producer = f.copy(0, &[]).unwrap();
            let dependency = f.context.record_event(&producer).unwrap();
            let mut consumer = f.copy(1, &[dependency]).unwrap();
            let event = f.context.record_event(&consumer).unwrap();
            f.context.release_event(dependency).unwrap();
            if cancel {
                f.context.backend.cancel_before_publication = true;
                assert_eq!(
                    f.context.cancel(&mut consumer).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
                assert_eq!(
                    f.context.cancel(&mut consumer).unwrap(),
                    RuntimeCancellationV1::TooLate
                );
            } else {
                f.context.wait_event(event, Duration::ZERO).unwrap();
                f.context.poll_event(event).unwrap();
            }
            assert_eq!(f.retains(&producer), 0);
            f.context.poll(&mut consumer).unwrap();
            assert_eq!(f.retains(&producer), 0);
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_stream_destruction_does_not_invalidate_retained_dependency_identity() {
    for journal in [false, true] {
        for consumer_first in [false, true] {
            let mut f = Fixture::new(journal);
            let original = f.stream;
            let later = f
                .context
                .create_stream(f.context.devices()[1].id())
                .unwrap();
            f.stream = if consumer_first { later } else { original };
            let producer = f.copy(0, &[]).unwrap();
            let event = f.context.record_event(&producer).unwrap();
            f.stream = if consumer_first { original } else { later };
            let consumer = f.copy(1, &[event]).unwrap();
            f.context.release_event(event).unwrap();
            if !consumer_first {
                f.context.destroy_stream(original).unwrap();
                assert_eq!(f.retains(&producer), 1);
                f.context.check_scalar_peer_custody_v1(consumer.id).unwrap();
            }
            assert!(f.context.cleanup().is_complete());
            assert!(f.context.scalar_peer_copies.is_empty());
        }
    }
}

#[test]
fn missing_root_or_incomplete_alias_counts_seal_before_discharge() {
    for journal in [false, true] {
        for missing_root in [false, true] {
            let mut f = Fixture::new(journal);
            let producer = f.copy(0, &[]).unwrap();
            let a = f.context.record_event(&producer).unwrap();
            let b = f.context.record_event(&producer).unwrap();
            let mut consumer = f.copy(1, &[a, b]).unwrap();
            if missing_root {
                f.context.scalar_peer_copies.remove(&consumer.id);
            } else {
                f.context
                    .submissions
                    .get_mut(&producer.id)
                    .unwrap()
                    .dependency_retains = 1;
            }
            let before = f.retains(&producer);
            assert!(matches!(
                f.context.wait(&mut consumer, Duration::ZERO),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidBackendDescription
                ))
            ));
            assert!(f.context.is_terminal());
            assert_eq!(f.retains(&producer), before);
            assert_eq!(
                f.context.submissions[&consumer.id].status,
                RuntimeCompletionStatusV1::Pending
            );
            assert!(!f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn pending_source_is_still_rejected_before_identity_or_backend_entry() {
    let mut f = Fixture::new(true);
    let producer = f.copy(0, &[]).unwrap();
    let event = f.context.record_event(&producer).unwrap();
    let stream = f
        .context
        .create_stream(f.context.devices()[0].id())
        .unwrap();
    let destination = f
        .context
        .allocate(
            f.context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            64,
            16,
        )
        .unwrap();
    let before = (
        f.context.next_identity,
        f.context.backend.copy_call_count,
        f.context.scalar_peer_copies.len(),
    );
    assert!(matches!(
        f.context.peer_copy(
            stream,
            region(f.destinations[0], RuntimeAccessV1::Read, 24),
            region(destination, RuntimeAccessV1::Write, 0),
            &[event]
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(
        (
            f.context.next_identity,
            f.context.backend.copy_call_count,
            f.context.scalar_peer_copies.len()
        ),
        before
    );
    assert_eq!(f.retains(&producer), 0);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn first_nonjournaled_scalar_panic_activates_custody_guard() {
    let mut f = Fixture::new(false);
    assert!(f.context.scalar_peer_copies.is_empty());
    f.context.backend.copy_failure = MockMemoryFailure::Panic;
    let result = catch_unwind(AssertUnwindSafe(|| f.copy(0, &[])));
    assert!(result.is_err());
    assert!(f.context.is_terminal());
    assert!(f.context.submissions.is_empty());
    assert_eq!(f.context.cleanup().scalar_peer_copy_records_v1(), 1);
    assert!(f.context.backend.cleanup_log.is_empty());
}

#[test]
fn completion_failures_discharge_only_conclusive_consumer_quiescence() {
    for journal in [false, true] {
        for failure in [
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
            MockMemoryFailure::Terminal,
            MockMemoryFailure::Panic,
        ] {
            let mut f = Fixture::new(journal);
            let producer = f.copy(0, &[]).unwrap();
            let event = f.context.record_event(&producer).unwrap();
            let mut consumer = f.copy(1, &[event]).unwrap();
            f.context.release_event(event).unwrap();
            f.context.backend.batch_failure = failure;
            let result = catch_unwind(AssertUnwindSafe(|| {
                f.context
                    .wait_peer_copy_batch(&mut [&mut consumer], Duration::ZERO)
            }));
            match failure {
                MockMemoryFailure::Rejected => assert!(matches!(
                    result,
                    Ok(Err(RuntimeErrorV1::BackendRejected(_)))
                )),
                MockMemoryFailure::Quiescent => assert!(matches!(
                    result,
                    Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))
                )),
                MockMemoryFailure::Terminal => assert!(matches!(
                    result,
                    Ok(Err(RuntimeErrorV1::BackendTerminal(_)))
                )),
                MockMemoryFailure::Panic => assert!(result.is_err()),
                MockMemoryFailure::None => unreachable!(),
            }
            let terminal = matches!(
                failure,
                MockMemoryFailure::Terminal | MockMemoryFailure::Panic
            );
            assert_eq!(f.context.is_terminal(), terminal);
            assert_eq!(
                f.retains(&producer),
                usize::from(failure != MockMemoryFailure::Quiescent)
            );
            assert_eq!(
                f.context.scalar_peer_copies[&consumer.id].dependencies_held,
                failure != MockMemoryFailure::Quiescent
            );
            assert_eq!(
                f.context.query_submission(&producer).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            if terminal {
                let before = f.context.backend.cleanup_log.clone();
                assert!(!f.context.cleanup().is_complete());
                assert_eq!(f.context.backend.cleanup_log, before);
            } else {
                assert!(f.context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn partial_stream_cleanup_rejection_preserves_consumer_holds_until_retry() {
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let consumer_stream = f.stream;
        f.stream = f
            .context
            .create_stream(f.context.devices()[1].id())
            .unwrap();
        let producer = f.copy(0, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        f.stream = consumer_stream;
        let consumer = f.copy(1, &[event]).unwrap();
        f.context.release_event(event).unwrap();
        f.context.backend.cleanup_failure = MockCleanupFailure::RejectStreamOnce;
        let report = f.context.cleanup();
        assert!(!report.is_complete());
        assert!(!report.is_terminal());
        assert_eq!(report.scalar_peer_copy_records_v1(), 2);
        assert_eq!(f.retains(&producer), 1);
        assert!(f.context.scalar_peer_copies[&consumer.id].dependencies_held);
        assert!(f.context.submissions[&producer.id].quiescent);
        assert!(
            !f.context
                .backend
                .cleanup_log
                .iter()
                .any(|(kind, _)| *kind == MockCleanupKind::Submission)
        );
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn old_event_on_held_producer_stream_remains_a_legal_cross_stream_dependency() {
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let mut producer = f.copy(0, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        f.context.wait(&mut producer, Duration::ZERO).unwrap();
        let hold = f.context.hold_unpublished_stream_v1(f.stream).unwrap();
        f.stream = f
            .context
            .create_stream(f.context.devices()[1].id())
            .unwrap();
        let mut consumer = f.copy(1, &[event]).unwrap();
        assert_eq!(f.retains(&producer), 1);
        f.context.wait(&mut consumer, Duration::ZERO).unwrap();
        assert_eq!(f.retains(&producer), 0);
        f.context.release_unpublished_hold_v1(&hold).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn repeated_drain_and_callback_panic_do_not_repeat_discharge() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let producer = f.copy(0, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        let mut consumer = f.copy(1, &[event]).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        f.context
            .on_completion(&consumer, move |status| {
                assert_eq!(status, RuntimeCompletionStatusV1::Succeeded);
                observed.fetch_add(1, Ordering::SeqCst);
                panic!("scripted scalar completion callback panic");
            })
            .unwrap();
        for _ in 0..2 {
            f.context
                .drain(&mut consumer, Instant::now() + Duration::from_secs(1))
                .unwrap();
            assert_eq!(f.retains(&producer), 0);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(f.context.completion_callback_panic_count(), 1);
            assert!(!f.context.is_terminal());
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn retain_overflow_and_late_duplicate_reject_before_identity_or_backend_entry() {
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let producer = f.copy(0, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        let before = (f.context.next_identity, f.context.backend.copy_call_count);
        f.context
            .submissions
            .get_mut(&producer.id)
            .unwrap()
            .dependency_retains = usize::MAX;
        assert!(matches!(
            f.copy(1, &[event]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Capacity
            ))
        ));
        assert_eq!(
            (f.context.next_identity, f.context.backend.copy_call_count),
            before
        );
        f.context
            .submissions
            .get_mut(&producer.id)
            .unwrap()
            .dependency_retains = 0;
        let mut events = Vec::new();
        for _ in 0..MAX_RUNTIME_DEPENDENCIES_V1 {
            events.push(f.context.record_event(&producer).unwrap());
        }
        events[MAX_RUNTIME_DEPENDENCIES_V1 - 1] = events[0];
        let before = (f.context.next_identity, f.context.backend.copy_call_count);
        assert!(matches!(
            f.copy(1, &events),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::DuplicateDependency
            ))
        ));
        assert_eq!(
            (f.context.next_identity, f.context.backend.copy_call_count),
            before
        );
        assert_eq!(f.retains(&producer), 0);
        assert!(f.context.cleanup().is_complete());
    }
}
