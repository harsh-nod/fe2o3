use super::*;

#[test]
fn progress_registration_is_unique_bounded_and_context_checked() {
    let (mut context, state, first_stream, _event, _submission) = progress_fixture();
    let (second_stream, _, _) = append_submission_with_stream(&mut context, &state, 2, "second");
    let (mut foreign_context, foreign_state, _, _, _) = progress_fixture();
    let foreign_stream = foreign_context
        .create_stream(foreign_context.devices()[0].id())
        .unwrap();
    drop(foreign_state);

    let progress_config = RuntimeAsyncProgressConfigV1::new(1, 1).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        progress_config,
    )
    .unwrap();
    let first = handle.register_stream(first_stream).unwrap();
    assert!(matches!(
        handle.register_stream(first_stream),
        Err(RuntimeAsyncProgressRegistrationErrorV1::DuplicateStream)
    ));
    assert!(matches!(
        handle.register_stream(second_stream),
        Err(RuntimeAsyncProgressRegistrationErrorV1::Capacity)
    ));
    assert!(matches!(
        handle.register_stream(foreign_stream),
        Err(RuntimeAsyncProgressRegistrationErrorV1::InvalidStream(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    let nested = handle.clone();
    assert!(matches!(
        handle
            .observer()
            .try_with_context(move |_| nested.register_stream(second_stream))
            .unwrap(),
        Err(RuntimeAsyncProgressRegistrationErrorV1::ReentrantCall)
    ));
    drop(first);
    let second = handle.register_stream(second_stream).unwrap();
    drop(second);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn progress_scan_has_an_independent_budget_and_cyclic_cursor() {
    let (mut context, state, first_stream, _event, _submission) = progress_fixture();
    let (second_stream, _, _) = append_submission_with_stream(&mut context, &state, 2, "second");
    let (third_stream, _, _) = append_submission_with_stream(&mut context, &state, 3, "third");
    state.lock().unwrap().flush_outcomes.extend([
        MockFlushOutcome::Rejected("first"),
        MockFlushOutcome::Rejected("second"),
        MockFlushOutcome::Rejected("third"),
    ]);
    let cells = [
        (first_stream, Arc::new(RuntimeAsyncProgressCellV1::new())),
        (second_stream, Arc::new(RuntimeAsyncProgressCellV1::new())),
        (third_stream, Arc::new(RuntimeAsyncProgressCellV1::new())),
    ];
    let mut registrations = BTreeMap::from(cells.clone());
    let mut next_stream = None;
    for _ in 0..3 {
        assert!(!flush_progress_v1(
            &mut context,
            &mut registrations,
            &mut next_stream,
            1,
            flush_stream_v1::<MockBackend>,
        ));
    }
    let state = state.lock().unwrap();
    assert_eq!(
        state
            .flush_calls
            .iter()
            .map(|(stream, _)| *stream)
            .collect::<Vec<_>>(),
        state.created_streams
    );
    assert!(
        cells
            .iter()
            .all(|(_, cell)| { cell.state.lock().unwrap().failure_count == 1 })
    );
}

#[test]
fn progress_cursor_rotates_across_quiescent_abandoned_and_removed_streams() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let mut context = RuntimeContextV1::open(MockBackend {
        state: Arc::clone(&state),
    })
    .unwrap();
    let device = context.devices()[0].id();

    let first = context.create_stream(device).unwrap();
    append_submission_on_stream(&mut context, &state, first, 1, "first");
    let quiescent = context.create_stream(device).unwrap();
    let abandoned = context.create_stream(device).unwrap();
    append_submission_on_stream(&mut context, &state, abandoned, 2, "abandoned");
    let removed = context.create_stream(device).unwrap();
    let last = context.create_stream(device).unwrap();
    append_submission_on_stream(&mut context, &state, last, 3, "last");
    let backend_streams = state.lock().unwrap().created_streams.clone();
    context.destroy_stream(removed).unwrap();

    let first_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let quiescent_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let abandoned_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let removed_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let last_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let abandoned_registration = RuntimeAsyncProgressRegistrationV1 {
        stream: abandoned,
        cell: Arc::clone(&abandoned_cell),
    };
    let removed_registration = RuntimeAsyncProgressRegistrationV1 {
        stream: removed,
        cell: Arc::clone(&removed_cell),
    };
    let mut registrations = BTreeMap::from([
        (first, first_cell),
        (quiescent, Arc::clone(&quiescent_cell)),
        (abandoned, Arc::clone(&abandoned_cell)),
        (removed, Arc::clone(&removed_cell)),
        (last, last_cell),
    ]);
    drop(abandoned_registration);

    let mut next_stream = None;
    for _ in 0..6 {
        assert!(!flush_progress_v1(
            &mut context,
            &mut registrations,
            &mut next_stream,
            1,
            flush_stream_v1::<MockBackend>,
        ));
    }

    assert_eq!(
        state
            .lock()
            .unwrap()
            .flush_calls
            .iter()
            .map(|(stream, _)| *stream)
            .collect::<Vec<_>>(),
        vec![backend_streams[0], backend_streams[4], backend_streams[0]]
    );
    assert_eq!(next_stream, Some(quiescent));
    assert!(abandoned_cell.stopped.load(Ordering::Acquire));
    assert!(removed_registration.is_stopped());
    assert!(matches!(
        removed_registration.take_failure(),
        Some(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert_eq!(removed_registration.failure_count(), 1);
    assert!(!quiescent_cell.stopped.load(Ordering::Acquire));
    assert_eq!(registrations.len(), 3);
}

#[test]
fn retryable_progress_failures_are_retained_without_unregistering() {
    let (mut context, state, stream, _event, _submission) = progress_fixture();
    state.lock().unwrap().flush_outcomes.extend([
        MockFlushOutcome::Rejected("busy"),
        MockFlushOutcome::Quiescent("retry quiescent"),
        MockFlushOutcome::Success,
    ]);
    let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let registration = RuntimeAsyncProgressRegistrationV1 {
        stream,
        cell: Arc::clone(&cell),
    };
    let mut registrations = BTreeMap::from([(stream, cell)]);
    let mut next_stream = None;
    for _ in 0..3 {
        assert!(!flush_progress_v1(
            &mut context,
            &mut registrations,
            &mut next_stream,
            1,
            flush_stream_v1::<MockBackend>,
        ));
    }
    assert_eq!(registration.failure_count(), 2);
    assert!(matches!(
        registration.take_failure(),
        Some(RuntimeErrorV1::BackendRejected(MockError("busy")))
    ));
    assert!(!registration.is_stopped());
    assert!(registrations.contains_key(&stream));
}

#[test]
fn progress_failure_count_saturates_without_losing_the_retained_failure() {
    let cell = RuntimeAsyncProgressCellV1::new();
    cell.state.lock().unwrap().failure_count = u64::MAX - 1;

    cell.retain_failure(RuntimeErrorV1::BackendRejected(MockError("first")), false);
    assert_eq!(cell.state.lock().unwrap().failure_count, u64::MAX);
    cell.retain_failure(
        RuntimeErrorV1::BackendQuiescent(MockError("discarded")),
        false,
    );
    let mut state = cell.state.lock().unwrap();
    assert_eq!(state.failure_count, u64::MAX);
    assert!(matches!(
        state.failure.take(),
        Some(RuntimeErrorV1::BackendRejected(MockError("first")))
    ));
    drop(state);

    cell.retain_failure(RuntimeErrorV1::BackendTerminal(MockError("terminal")), true);
    let state = cell.state.lock().unwrap();
    assert_eq!(state.failure_count, u64::MAX);
    assert!(matches!(
        state.failure,
        Some(RuntimeErrorV1::BackendTerminal(MockError("terminal")))
    ));
}

#[test]
fn terminal_progress_failure_replaces_a_retained_retryable_failure() {
    let (mut context, state, stream, _event, _submission) = progress_fixture();
    state.lock().unwrap().flush_outcomes.extend([
        MockFlushOutcome::Rejected("busy"),
        MockFlushOutcome::Terminal("terminal"),
    ]);
    let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let registration = RuntimeAsyncProgressRegistrationV1 {
        stream,
        cell: Arc::clone(&cell),
    };
    let mut registrations = BTreeMap::from([(stream, cell)]);
    let mut next_stream = None;
    assert!(!flush_progress_v1(
        &mut context,
        &mut registrations,
        &mut next_stream,
        1,
        flush_stream_v1::<MockBackend>,
    ));
    assert!(flush_progress_v1(
        &mut context,
        &mut registrations,
        &mut next_stream,
        1,
        flush_stream_v1::<MockBackend>,
    ));
    assert_eq!(registration.failure_count(), 2);
    assert!(matches!(
        registration.take_failure(),
        Some(RuntimeErrorV1::BackendTerminal(MockError("terminal")))
    ));
    assert!(registration.is_stopped());
    assert!(context.is_terminal());
}

#[test]
fn dropping_progress_observers_never_cancels_or_releases_work() {
    let (context, state, stream, event, submission) = progress_fixture();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let registration = handle.register_stream(stream).unwrap();
    drop(handle.observer().event_future(event).unwrap());
    wait_until(|| !state.lock().unwrap().flush_calls.is_empty());
    assert_eq!(state.lock().unwrap().release_calls, 0);
    assert_eq!(
        state.lock().unwrap().statuses.get(&submission),
        Some(&BackendPollV1::Pending)
    );
    drop(registration);
    drop(handle);
    let _context = engine.into_context().unwrap();
    assert_eq!(state.lock().unwrap().release_calls, 0);
}

#[test]
fn dropping_registration_removes_it_without_a_final_flush() {
    let (mut context, state, stream, _event, _submission) = progress_fixture();
    let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let registration = RuntimeAsyncProgressRegistrationV1 {
        stream,
        cell: Arc::clone(&cell),
    };
    let mut registrations = BTreeMap::from([(stream, cell)]);
    drop(registration);
    assert!(!flush_progress_v1(
        &mut context,
        &mut registrations,
        &mut None,
        1,
        flush_stream_v1::<MockBackend>,
    ));
    assert!(registrations.is_empty());
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert_eq!(state.lock().unwrap().release_calls, 0);
}

#[test]
fn dropping_registration_during_a_claimed_flush_allows_only_that_flush() {
    let (context, state, stream, _event, _submission) = progress_fixture();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    state.lock().unwrap().flush_barriers = Some((Arc::clone(&entered), Arc::clone(&release)));
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let registration = handle.register_stream(stream).unwrap();

    entered.wait();
    drop(registration);
    release.wait();
    handle.observer().try_with_context(|_| ()).unwrap();
    drop(handle);
    let _context = engine.into_context().unwrap();

    let state = state.lock().unwrap();
    assert_eq!(state.flush_calls.len(), 1);
    assert_eq!(state.release_calls, 0);
}

#[test]
fn queued_stop_with_active_registration_performs_no_final_backend_call() {
    let (context, state, stream, _event, submission) = progress_fixture();
    let config = RuntimeAsyncEngineConfigV1::default();
    let progress_config = RuntimeAsyncProgressConfigV1::default();
    let (sender, receiver) = sync_channel(config.command_capacity);
    let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let (response_sender, response_receiver) = sync_channel(1);
    sender
        .try_send(RuntimeAsyncEngineCommandV1::RegisterProgress {
            stream,
            cell: Arc::clone(&cell),
            response: response_sender.into(),
        })
        .unwrap();
    sender.try_send(RuntimeAsyncEngineCommandV1::Stop).unwrap();
    drop(sender);

    let context = run_engine_v1(
        context,
        receiver,
        config,
        Some(RuntimeAsyncProgressModeV1 {
            config: progress_config,
            flush_stream: flush_stream_v1::<MockBackend>,
        }),
        drain::AdmissionV1::new(),
    );
    assert_eq!(response_receiver.recv().unwrap(), Ok(()));
    assert!(cell.stopped.load(Ordering::Acquire));
    let state = state.lock().unwrap();
    assert!(state.flush_calls.is_empty());
    assert_eq!(state.release_calls, 0);
    assert_eq!(
        state.statuses.get(&submission),
        Some(&BackendPollV1::Pending)
    );
    drop(state);
    assert!(!context.is_terminal());
}

#[test]
fn stop_beyond_the_command_budget_takes_effect_at_the_next_tick_boundary() {
    let (context, state, stream, _event, submission) = progress_fixture();
    let config = RuntimeAsyncEngineConfigV1::new(8, 8, 1, 1, Duration::from_millis(1)).unwrap();
    let progress_config = RuntimeAsyncProgressConfigV1::default();
    let (sender, receiver) = sync_channel(config.command_capacity);
    let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let (response_sender, response_receiver) = sync_channel(1);
    sender
        .try_send(RuntimeAsyncEngineCommandV1::RegisterProgress {
            stream,
            cell: Arc::clone(&cell),
            response: response_sender.into(),
        })
        .unwrap();
    sender.try_send(RuntimeAsyncEngineCommandV1::Stop).unwrap();
    drop(sender);

    let context = run_engine_v1(
        context,
        receiver,
        config,
        Some(RuntimeAsyncProgressModeV1 {
            config: progress_config,
            flush_stream: flush_stream_v1::<MockBackend>,
        }),
        drain::AdmissionV1::new(),
    );

    assert_eq!(response_receiver.recv().unwrap(), Ok(()));
    assert!(cell.stopped.load(Ordering::Acquire));
    let state = state.lock().unwrap();
    assert_eq!(state.flush_calls.len(), 1);
    assert_eq!(state.release_calls, 0);
    assert_eq!(
        state.statuses.get(&submission),
        Some(&BackendPollV1::Pending)
    );
    drop(state);
    assert!(!context.is_terminal());
}

#[test]
fn destroyed_registered_stream_retains_validation_failure_and_stops() {
    let (mut context, _state, stream, _event, _submission) = progress_fixture();
    let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let registration = RuntimeAsyncProgressRegistrationV1 {
        stream,
        cell: Arc::clone(&cell),
    };
    let mut registrations = BTreeMap::from([(stream, cell)]);
    context.destroy_stream(stream).unwrap();
    assert!(!flush_progress_v1(
        &mut context,
        &mut registrations,
        &mut None,
        1,
        flush_stream_v1::<MockBackend>,
    ));
    assert!(matches!(
        registration.take_failure(),
        Some(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert!(registration.is_stopped());
    assert!(registrations.is_empty());
}

#[test]
fn terminal_progress_failure_is_exact_and_seals_all_engine_activity() {
    let (mut context, state, first_stream, event, _submission) = progress_fixture();
    let (second_stream, _, _) = append_submission_with_stream(&mut context, &state, 2, "second");
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let mut future = handle.observer().event_future(event).unwrap();
    let first = handle.register_stream(first_stream).unwrap();
    let second = handle.register_stream(second_stream).unwrap();
    state
        .lock()
        .unwrap()
        .flush_outcomes
        .push_back(MockFlushOutcome::Terminal("sealed"));
    wait_until(|| first.is_stopped() && second.is_stopped());
    let failures = [first.take_failure(), second.take_failure()];
    assert_eq!(
        failures
            .iter()
            .filter(|failure| matches!(
                failure,
                Some(RuntimeErrorV1::BackendTerminal(MockError("sealed")))
            ))
            .count(),
        1
    );
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_until_ready(&mut future, &waker),
        Err(RuntimeAsyncEventErrorV1::EngineStopped)
    ));
    let calls_after_seal = state.lock().unwrap().flush_calls.len();
    thread::sleep(Duration::from_millis(5));
    assert_eq!(state.lock().unwrap().flush_calls.len(), calls_after_seal);
    drop(handle);
    let context = engine.into_context().unwrap();
    assert!(context.is_terminal());
    assert_eq!(state.lock().unwrap().release_calls, 0);
}

#[test]
fn terminal_event_poll_stops_progress_before_the_flush_phase() {
    let (context, state, stream, event, _submission) = progress_fixture();
    state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Terminal(MockError("poll sealed")));
    let config = RuntimeAsyncEngineConfigV1::default();
    let progress_config = RuntimeAsyncProgressConfigV1::default();
    let (sender, receiver) = sync_channel(config.command_capacity);
    let progress_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let (progress_response_sender, progress_response_receiver) = sync_channel(1);
    sender
        .try_send(RuntimeAsyncEngineCommandV1::RegisterProgress {
            stream,
            cell: Arc::clone(&progress_cell),
            response: progress_response_sender.into(),
        })
        .unwrap();
    let future_cell = Arc::new(RuntimeAsyncFutureCellV1::new());
    let (future_response_sender, future_response_receiver) = sync_channel(1);
    sender
        .try_send(RuntimeAsyncEngineCommandV1::Register {
            event,
            cell: Arc::clone(&future_cell),
            response: future_response_sender.into(),
        })
        .unwrap();

    let context = run_engine_v1(
        context,
        receiver,
        config,
        Some(RuntimeAsyncProgressModeV1 {
            config: progress_config,
            flush_stream: flush_stream_v1::<MockBackend>,
        }),
        drain::AdmissionV1::new(),
    );
    drop(sender);
    assert_eq!(progress_response_receiver.recv().unwrap(), Ok(()));
    assert_eq!(future_response_receiver.recv().unwrap(), Ok(()));
    assert!(progress_cell.stopped.load(Ordering::Acquire));
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert!(context.is_terminal());

    let mut future = RuntimeEventFutureV1 {
        event,
        cell: future_cell,
        completed: false,
    };
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_once(&mut future, &waker),
        Poll::Ready(Err(RuntimeAsyncEventErrorV1::Runtime(
            RuntimeErrorV1::BackendTerminal(MockError("poll sealed"))
        )))
    ));
}

#[test]
fn event_poll_and_progress_flush_share_one_worker_thread() {
    let (context, state, stream, event, _submission) = progress_fixture();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let _future = handle.observer().event_future(event).unwrap();
    let registration = handle.register_stream(stream).unwrap();
    wait_until(|| {
        let state = state.lock().unwrap();
        !state.poll_threads.is_empty() && !state.flush_calls.is_empty()
    });
    let state_guard = state.lock().unwrap();
    assert_eq!(state_guard.poll_threads.len(), 1);
    assert!(
        state_guard
            .flush_calls
            .iter()
            .all(|(_, worker)| state_guard.poll_threads.contains(worker))
    );
    drop(state_guard);
    drop(registration);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn progress_handle_and_worker_v4_v5_paths_are_send_compatible() {
    fn require_send<T: Send>() {}
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<RuntimeAsyncProgressHandleV1<MockBackend>>();
    require_send::<crate::RuntimeWorkerBackendV4<crate::RuntimeBinaryCodecV4>>();
    require_send::<crate::RuntimeWorkerBackendV5<crate::RuntimeBinaryCodecV5>>();
}
