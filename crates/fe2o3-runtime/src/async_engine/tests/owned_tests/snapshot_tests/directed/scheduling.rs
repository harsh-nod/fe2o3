use super::*;

fn ready<F: Future + Unpin>(mut future: F) -> F::Output {
    match Pin::new(&mut future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("scheduler must have acknowledged registration"),
    }
}

#[test]
fn directed_async_explicit_same_stream_registration_has_independent_flush_budget() {
    for registration_first in [false, true] {
        for paired in [false, true] {
            let mut f = Fixture::new(true);
            let (_producer, event, _) = f.producer(0, 1, &[]);
            // An independent observer on the consumer stream may flush it.
            let (_, observer_event, _) = f.producer(0, 3, &[]);
            let (ordinary_observer, paired_observer);
            let mut future = None;
            if !registration_first {
                future = Some(f.enqueue(2, 5, vec![], false).0);
            }
            if paired {
                ordinary_observer = None;
                paired_observer = Some(
                    f.h.handle
                        .enqueue_event_registration_with_progress(f.streams[1], observer_event)
                        .unwrap(),
                );
            } else {
                paired_observer = None;
                ordinary_observer = Some(
                    f.h.handle
                        .enqueue_stream_registration(f.streams[1])
                        .unwrap(),
                );
            }
            if registration_first {
                future = Some(f.enqueue(2, 5, vec![], false).0);
            }
            let mut registry = operation::OperationRegistryV1::new(8, false);
            let mut scheduler =
                scheduler::SchedulerV1::new(f.h.handle.observer.admission.clone(), true, false);
            let mode = RuntimeAsyncProgressModeV1 {
                config: RuntimeAsyncProgressConfigV1::new(8, 1).unwrap(),
                flush_stream: |context, stream| context.flush_stream(stream),
            };
            scheduler.tick(
                &mut f.h.context,
                &mut registry,
                &f.h.receiver,
                RuntimeAsyncEngineConfigV1::default(),
                Some(&mode),
                Duration::ZERO,
            );
            let registration = ordinary_observer.map(|pending| ready(pending).unwrap().unwrap());
            let paired_registration =
                paired_observer.map(|pending| ready(pending).unwrap().unwrap());
            let consumer = f.last_id();
            assert_eq!(f.h.state.lock().unwrap().flush_calls.len(), 1);
            {
                let state = f.h.state.lock().unwrap();
                assert_eq!(
                    state.flush_calls[0].0,
                    state.directed_routes[&consumer].0.stream
                );
            }
            f.complete();
            scheduler.tick(
                &mut f.h.context,
                &mut registry,
                &f.h.receiver,
                RuntimeAsyncEngineConfigV1::default(),
                Some(&mode),
                Duration::ZERO,
            );
            assert_eq!(
                join_command(future.unwrap()).unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(
                f.h.state.lock().unwrap().flush_calls.len(),
                if paired { 1 } else { 2 }
            );
            drop((registration, paired_registration));
            scheduler.finish(&mut f.h.context, &mut registry);
            f.h.context.release_event(event).unwrap();
            assert!(f.h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn directed_async_multiple_drivers_each_advance_once_without_automatic_flush() {
    let mut f = Fixture::new(true);
    let (first, _) = f.enqueue(0, 1, vec![], false);
    let (second, _) = f.enqueue(2, 3, vec![], true);
    let mut registry = operation::OperationRegistryV1::new(4, false);
    registry.insert(f.h.pop());
    registry.insert(f.h.pop());
    f.tick(&mut registry);
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 2);
    f.tick(&mut registry);
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 4);
    assert_eq!(registry.len(), 2);
    f.complete();
    f.tick(&mut registry);
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 6);
    assert_eq!(registry.len(), 0);
    for future in [first, second] {
        assert_eq!(
            join_command(future).unwrap().observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
    }
    assert!(f.h.state.lock().unwrap().flush_calls.is_empty());
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn directed_async_driver_retires_locally_after_ordinary_stream_observation() {
    let mut f = Fixture::new(true);
    let (_, event, _) = f.producer(0, 1, &[]);
    let (future, _) = f.enqueue(1, 2, vec![event], false);
    let mut driver = f.h.pop();
    assert!(!driver.advance(&mut f.h.context));
    f.complete();
    for _ in 0..2 {
        f.h.context
            .synchronize_stream(f.streams[0], Duration::ZERO)
            .unwrap();
    }
    let count = f.h.state.lock().unwrap().directed_calls.len();
    assert_eq!(count, 4);
    assert!(driver.advance(&mut f.h.context));
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), count);
    assert_eq!(
        join_command(future).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn directed_async_context_returning_engine_preserves_exact_submission() {
    let state = Arc::new(Mutex::new(MockState {
        peer_devices: true,
        ..MockState::default()
    }));
    let mut context = RuntimeContextV1::open_with_version_journal_v1(
        MockBackend {
            state: state.clone(),
        },
        16,
        8,
    )
    .unwrap();
    let (stream, source, destination, event) = seed(&mut context);
    let (mut engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap_or_else(|_| panic!("engine spawn"));
    let operation = handle
        .directed_peer_copy_tracked(stream, source, destination, vec![event])
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while matches!(
        operation.control.phase(),
        RuntimeAsyncOperationPhaseV1::Queued | RuntimeAsyncOperationPhaseV1::SubmissionStarted
    ) {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert_eq!(
        operation.control.phase(),
        RuntimeAsyncOperationPhaseV1::Observing
    );
    for status in state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    let result = join_command(operation.future).unwrap();
    let mut context = engine.stop_and_join().unwrap();
    assert_eq!(
        result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        context
            .query_submission(&result.submission.unwrap())
            .unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert!(context.cleanup().is_complete());
}

fn seed<B: RuntimeDirectedScalarPeerCopyBackendV1>(
    context: &mut RuntimeContextV1<B>,
) -> (
    RuntimeStreamIdV1,
    RuntimeMemoryRegionV1,
    RuntimeMemoryRegionV1,
    RuntimeEventIdV1,
)
where
    B::Error: std::fmt::Debug,
{
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let streams = [
        context.create_stream(devices[0]).unwrap(),
        context.create_stream(devices[1]).unwrap(),
    ];
    let regions: Vec<_> = (0..3)
        .map(|index| RuntimeMemoryRegionV1 {
            allocation: context
                .allocate(devices[index % 2], RuntimeMemoryKindV1::DeviceLocal, 16, 8)
                .unwrap(),
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        })
        .collect();
    let producer = context
        .directed_peer_copy_v1(
            streams[1],
            regions[0],
            RuntimeMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..regions[1]
            },
            &[],
        )
        .unwrap();
    let event = context.record_event(&producer).unwrap();
    (
        streams[0],
        regions[1],
        RuntimeMemoryRegionV1 {
            access: RuntimeAccessV1::Write,
            ..regions[2]
        },
        event,
    )
}

#[test]
fn directed_async_current_thread_owner_keeps_all_native_calls_on_creator() {
    let state = Arc::new(Mutex::new(MockState {
        peer_devices: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || {
            RuntimeContextV1::open_with_version_journal_v1(
                ThreadBoundBackend {
                    inner: MockBackend {
                        state: state.clone(),
                    },
                    owner: thread::current().id(),
                    local: Rc::new(Cell::new(0)),
                    trace: trace.clone(),
                },
                16,
                8,
            )
        },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let seed_command = handle.observer().enqueue_with_context(seed).unwrap();
    let (stream, source, destination, event) =
        current_thread_tests::drive(&mut engine, seed_command).unwrap();
    let future = handle
        .directed_peer_copy(stream, source, destination, vec![event])
        .unwrap();
    assert_eq!(engine.tick(), Ok(RuntimeAsyncTickV1::Running));
    for status in state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    let result = current_thread_tests::drive(&mut engine, future).unwrap();
    assert_eq!(
        result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert!(
        trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .all(|(_, owner)| *owner == thread::current().id())
    );
    assert!(
        trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(name, _)| *name == "directed_progress")
    );
    assert!(state.lock().unwrap().flush_calls.is_empty());
}

#[test]
fn directed_async_background_non_send_owner_progresses_and_cleans_up() {
    let state = Arc::new(Mutex::new(MockState {
        peer_devices: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let (stream, source, destination, event) =
        join_command(handle.observer().enqueue_with_context(seed).unwrap()).unwrap();
    let future = handle
        .directed_peer_copy_tracked(stream, source, destination, vec![event])
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while matches!(
        future.control.phase(),
        RuntimeAsyncOperationPhaseV1::Queued | RuntimeAsyncOperationPhaseV1::SubmissionStarted
    ) {
        assert!(Instant::now() < deadline, "owner did not submit");
        thread::yield_now();
    }
    assert_eq!(
        future.control.phase(),
        RuntimeAsyncOperationPhaseV1::Observing
    );
    for status in state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    let result = join_command(future.future).unwrap();
    assert_eq!(
        result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    let trace = trace.lock().unwrap();
    let owner = trace.calls[0].1;
    assert_ne!(owner, thread::current().id());
    assert!(trace.calls.iter().all(|(_, id)| *id == owner));
    assert!(
        trace
            .calls
            .iter()
            .any(|(name, _)| *name == "directed_progress")
    );
    assert!(state.lock().unwrap().flush_calls.is_empty());
}

#[test]
fn early_event_current_thread_non_send_owner_records_before_completion() {
    let state = Arc::new(Mutex::new(MockState {
        peer_devices: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || {
            RuntimeContextV1::open_with_version_journal_v1(
                ThreadBoundBackend {
                    inner: MockBackend {
                        state: state.clone(),
                    },
                    owner: thread::current().id(),
                    local: Rc::new(Cell::new(0)),
                    trace: trace.clone(),
                },
                16,
                8,
            )
        },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let command = handle.observer().enqueue_with_context(seed).unwrap();
    let (stream, source, destination, parent) =
        current_thread_tests::drive(&mut engine, command).unwrap();
    let early = handle
        .directed_peer_copy_with_event(stream, source, destination, vec![parent])
        .unwrap();
    let event = current_thread_tests::drive(&mut engine, early.event)
        .unwrap()
        .unwrap();
    let command = handle
        .observer()
        .enqueue_with_context(move |context| {
            assert_eq!(
                context.query_event(event).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            context.release_event(parent).unwrap();
        })
        .unwrap();
    current_thread_tests::drive(&mut engine, command).unwrap();
    for status in state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    assert_eq!(
        current_thread_tests::drive(&mut engine, early.operation)
            .unwrap()
            .observation
            .unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    let trace = trace.lock().unwrap();
    assert!(
        trace
            .calls
            .iter()
            .all(|(_, owner)| *owner == thread::current().id())
    );
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(name, _)| *name == "record_event_v1")
            .count(),
        2
    );
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn early_event_background_non_send_owner_records_before_completion() {
    let state = Arc::new(Mutex::new(MockState {
        peer_devices: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let (stream, source, destination, parent) =
        join_command(handle.observer().enqueue_with_context(seed).unwrap()).unwrap();
    let early = handle
        .directed_peer_copy_with_event(stream, source, destination, vec![parent])
        .unwrap();
    let event = join_command(early.event).unwrap().unwrap();
    join_command(
        handle
            .observer()
            .enqueue_with_context(move |context| {
                assert_eq!(
                    context.query_event(event).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
                context.release_event(parent).unwrap();
            })
            .unwrap(),
    )
    .unwrap();
    for status in state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    assert_eq!(
        join_command(early.operation).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    let trace = trace.lock().unwrap();
    assert!(
        trace
            .calls
            .iter()
            .all(|(_, owner)| *owner != thread::current().id())
    );
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(name, _)| *name == "record_event_v1")
            .count(),
        2
    );
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn early_event_drain_keeps_events_and_never_invents_unsubmitted_descendants() {
    for completed in [false, true] {
        let state = Arc::new(Mutex::new(MockState {
            peer_devices: true,
            ..MockState::default()
        }));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
            || {
                RuntimeContextV1::open_with_version_journal_v1(
                    ThreadBoundBackend {
                        inner: MockBackend {
                            state: state.clone(),
                        },
                        owner: thread::current().id(),
                        local: Rc::new(Cell::new(0)),
                        trace: trace.clone(),
                    },
                    16,
                    8,
                )
            },
            RuntimeAsyncEngineConfigV1::new(8, 8, 1, 1, Duration::from_millis(1)).unwrap(),
            RuntimeAsyncProgressConfigV1::new(8, 1).unwrap(),
        )
        .unwrap();
        let command = handle.observer().enqueue_with_context(seed).unwrap();
        let (stream, source, destination, parent) =
            current_thread_tests::drive(&mut engine, command).unwrap();
        let early = handle
            .directed_peer_copy_with_event(stream, source, destination, vec![parent])
            .unwrap();
        let event = current_thread_tests::drive(&mut engine, early.event)
            .unwrap()
            .unwrap();
        if completed {
            for status in state.lock().unwrap().statuses.values_mut() {
                *status = BackendPollV1::Succeeded;
            }
        }
        let drain = handle.begin_drain(if completed { 32 } else { 1 }).unwrap();
        assert!(matches!(
            handle.directed_peer_copy_with_event(stream, source, destination, vec![event]),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        assert_eq!(handle.observer().reply_cells_in_use(), 1);
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        let report = current_thread_tests::drive(&mut engine, drain).unwrap();
        assert_eq!(
            report.outcome,
            if completed {
                RuntimeAsyncDrainOutcomeV1::Quiescent
            } else {
                RuntimeAsyncDrainOutcomeV1::BudgetExhausted
            }
        );
        assert_eq!(report.retained_submissions.total_submissions, 2);
        assert_eq!(
            report.retained_submissions.succeeded,
            if completed { 2 } else { 0 }
        );
        assert_eq!(state.lock().unwrap().event_release_calls, 0);
        assert_eq!(state.lock().unwrap().release_calls, 0);
        let shutdown = engine.shutdown();
        assert_eq!(
            shutdown.disposition,
            if completed {
                RuntimeAsyncOwnedDispositionV1::Released
            } else {
                RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
            }
        );
        if completed {
            assert_eq!(
                join_command(early.operation).unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(state.lock().unwrap().event_release_calls, 2);
        } else {
            assert!(matches!(
                join_command(early.operation),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
            assert_eq!(shutdown.cleanup.unwrap().retained().events, 2);
            assert_eq!(state.lock().unwrap().event_release_calls, 0);
        }
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
    }
}
