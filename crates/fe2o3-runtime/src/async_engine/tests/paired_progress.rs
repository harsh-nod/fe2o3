use super::*;

#[test]
fn progress_engine_flushes_only_registered_pending_streams() {
    let (mut context, state, first_stream, _event, _submission) = progress_fixture();
    let (second_stream, _, _) = append_submission_with_stream(&mut context, &state, 2, "second");
    let backend_streams = state.lock().unwrap().created_streams.clone();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let registration = handle.register_stream(first_stream).unwrap();
    wait_until(|| !state.lock().unwrap().flush_calls.is_empty());
    assert!(
        state
            .lock()
            .unwrap()
            .flush_calls
            .iter()
            .all(|(stream, _)| *stream == backend_streams[0])
    );
    assert_ne!(first_stream, second_stream);
    drop(registration);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn idle_registration_starts_flushing_after_a_later_submission() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let mut context = RuntimeContextV1::open(MockBackend {
        state: Arc::clone(&state),
    })
    .unwrap();
    let stream = context.create_stream(context.devices()[0].id()).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let registration = handle.register_stream(stream).unwrap();
    thread::sleep(Duration::from_millis(10));
    assert!(state.lock().unwrap().flush_calls.is_empty());

    let submission_state = Arc::clone(&state);
    handle
        .observer()
        .try_with_context(move |context| {
            append_submission_on_stream(context, &submission_state, stream, 1, "later")
        })
        .unwrap();
    wait_until(|| !state.lock().unwrap().flush_calls.is_empty());

    drop(registration);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn paired_registration_rolls_back_both_sides_on_capacity_and_duplicates() {
    {
        let (context, _state, stream, event, _submission) = progress_fixture();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let event_future = handle.observer().event_future(event).unwrap();
        assert!(matches!(
            handle.event_future_with_progress(stream, event),
            Err(RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateEvent)
        ));
        let progress = handle.register_stream(stream).unwrap();
        drop(progress);
        drop(event_future);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    {
        let (context, _state, stream, event, _submission) = progress_fixture();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let progress = handle.register_stream(stream).unwrap();
        assert!(matches!(
            handle.event_future_with_progress(stream, event),
            Err(RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateStream)
        ));
        let event_future = handle.observer().event_future(event).unwrap();
        drop(event_future);
        drop(progress);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    {
        let (mut context, state, first_stream, first_event, _submission) = progress_fixture();
        let (second_stream, second_event, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let engine_config =
            RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            engine_config,
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let first = handle.observer().event_future(first_event).unwrap();
        assert!(matches!(
            handle.event_future_with_progress(second_stream, second_event),
            Err(RuntimeAsyncProgressEventRegistrationErrorV1::EventCapacity)
        ));
        let second_progress = handle.register_stream(second_stream).unwrap();
        assert_ne!(first_stream, second_stream);
        drop(second_progress);
        drop(first);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    {
        let (mut context, state, first_stream, _first_event, _submission) = progress_fixture();
        let (second_stream, second_event, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
        )
        .unwrap();
        let first = handle.register_stream(first_stream).unwrap();
        assert!(matches!(
            handle.event_future_with_progress(second_stream, second_event),
            Err(RuntimeAsyncProgressEventRegistrationErrorV1::ProgressCapacity)
        ));
        let second_event_future = handle.observer().event_future(second_event).unwrap();
        drop(second_event_future);
        drop(first);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }
}

#[test]
fn paired_registration_rejects_valid_wrong_stream_without_consuming_capacity() {
    let (mut context, state, first_stream, first_event, _submission) = progress_fixture();
    let (second_stream, _second_event, _) =
        append_submission_with_stream(&mut context, &state, 2, "second");
    let engine_config =
        RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        engine_config,
        RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
    )
    .unwrap();

    assert!(matches!(
        handle.event_future_with_progress(second_stream, first_event),
        Err(RuntimeAsyncProgressEventRegistrationErrorV1::EventStreamMismatch)
    ));
    let correct = handle
        .event_future_with_progress(first_stream, first_event)
        .unwrap();

    drop(correct);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn paired_progress_polls_63_packet_window_before_flushing_2_packet_continuation() {
    let (context, state, stream, event, submission) = progress_fixture();
    state.lock().unwrap().window_progress = Some(MockWindowProgressV1 {
        submission,
        window_packet_counts: VecDeque::from([63, 2]),
        published: true,
        continuation_ready: false,
    });
    let config = RuntimeAsyncEngineConfigV1::new(16, 16, 16, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        config,
        RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
    )
    .unwrap();
    let mut future = handle.event_future_with_progress(stream, event).unwrap();
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_until_ready(&mut future, &waker),
        Ok(RuntimeCompletionStatusV1::Succeeded)
    ));
    assert_eq!(future.progress_failure_count(), 0);
    let state = state.lock().unwrap();
    assert_eq!(
        state.progress_steps,
        [
            MockProgressStepV1::Poll(63),
            MockProgressStepV1::Flush(2),
            MockProgressStepV1::Poll(2)
        ]
    );
    assert_eq!(state.flush_calls.len(), 1);
    drop(state);
    drop(future);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn completed_paired_future_stops_flushing_and_reuses_both_capacities() {
    let (mut context, state, first_stream, first_event, first_submission) = progress_fixture();
    let (second_stream, second_event, _) =
        append_submission_with_stream(&mut context, &state, 2, "second");
    let engine_config =
        RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        engine_config,
        RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
    )
    .unwrap();
    let mut completed = handle
        .event_future_with_progress(first_stream, first_event)
        .unwrap();
    state
        .lock()
        .unwrap()
        .statuses
        .insert(first_submission, BackendPollV1::Succeeded);
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_until_ready(&mut completed, &waker),
        Ok(RuntimeCompletionStatusV1::Succeeded)
    ));
    assert!(completed.is_progress_stopped());
    let flush_count = state.lock().unwrap().flush_calls.len();
    thread::sleep(Duration::from_millis(10));
    assert_eq!(state.lock().unwrap().flush_calls.len(), flush_count);

    let replacement = handle
        .event_future_with_progress(second_stream, second_event)
        .unwrap();
    assert_eq!(replacement.event(), second_event);
    assert_eq!(replacement.stream(), second_stream);
    drop(replacement);
    drop(completed);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn paired_event_poll_error_stops_flushing_and_reuses_both_capacities() {
    let (context, state, stream, event, _submission) = progress_fixture();
    state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError("poll")));
    let engine_config =
        RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        engine_config,
        RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
    )
    .unwrap();
    let mut completed = handle.event_future_with_progress(stream, event).unwrap();
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_until_ready(&mut completed, &waker),
        Err(RuntimeAsyncEventErrorV1::Runtime(
            RuntimeErrorV1::BackendRejected(MockError("poll"))
        ))
    ));
    assert!(completed.is_progress_stopped());
    assert!(state.lock().unwrap().flush_calls.is_empty());

    let replacement = handle.event_future_with_progress(stream, event).unwrap();
    drop(replacement);
    drop(completed);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn terminal_paired_event_bypasses_unused_registry_capacity_checks() {
    {
        let (mut context, state, _first_stream, first_event, _first_submission) =
            progress_fixture();
        let (second_stream, second_event, second_submission) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        state
            .lock()
            .unwrap()
            .statuses
            .insert(second_submission, BackendPollV1::Succeeded);
        assert_eq!(
            context.poll_event(second_event).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        let engine_config =
            RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            engine_config,
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let pending = handle.observer().event_future(first_event).unwrap();
        let mut terminal = handle
            .event_future_with_progress(second_stream, second_event)
            .unwrap();
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_once(&mut terminal, &waker),
            Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
        ));
        assert!(terminal.is_progress_stopped());
        drop(terminal);
        drop(pending);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    {
        let (mut context, state, first_stream, _first_event, _first_submission) =
            progress_fixture();
        let (second_stream, second_event, second_submission) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        state
            .lock()
            .unwrap()
            .statuses
            .insert(second_submission, BackendPollV1::Succeeded);
        assert_eq!(
            context.poll_event(second_event).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
        )
        .unwrap();
        let progress = handle.register_stream(first_stream).unwrap();
        let mut terminal = handle
            .event_future_with_progress(second_stream, second_event)
            .unwrap();
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_once(&mut terminal, &waker),
            Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
        ));
        assert!(terminal.is_progress_stopped());
        drop(terminal);
        drop(progress);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }
}

#[test]
fn paired_progress_retains_retryable_flush_failure() {
    let (context, state, stream, event, _submission) = progress_fixture();
    state
        .lock()
        .unwrap()
        .flush_outcomes
        .push_back(MockFlushOutcome::Rejected("retry"));
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let future = handle.event_future_with_progress(stream, event).unwrap();
    wait_until(|| future.progress_failure_count() != 0);
    assert!(matches!(
        future.take_progress_failure(),
        Some(RuntimeErrorV1::BackendRejected(MockError("retry")))
    ));
    assert!(!future.is_progress_stopped());
    drop(future);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn dropping_paired_future_frees_both_registries_without_release() {
    let (context, state, stream, event, submission) = progress_fixture();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let future = handle.event_future_with_progress(stream, event).unwrap();
    drop(future);
    let replacement = handle.event_future_with_progress(stream, event).unwrap();
    assert_eq!(replacement.event(), event);
    assert_eq!(replacement.stream(), stream);
    drop(replacement);
    drop(handle);
    let _context = engine.into_context().unwrap();
    let state = state.lock().unwrap();
    assert_eq!(state.release_calls, 0);
    assert_eq!(
        state.statuses.get(&submission),
        Some(&BackendPollV1::Pending)
    );
}

#[test]
fn paired_progress_terminal_failure_seals_future_and_registration() {
    let (context, state, stream, event, _submission) = progress_fixture();
    state
        .lock()
        .unwrap()
        .flush_outcomes
        .push_back(MockFlushOutcome::Terminal("sealed"));
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let mut future = handle.event_future_with_progress(stream, event).unwrap();
    wait_until(|| future.is_progress_stopped());
    assert!(matches!(
        future.take_progress_failure(),
        Some(RuntimeErrorV1::BackendTerminal(MockError("sealed")))
    ));
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_until_ready(&mut future, &waker),
        Err(RuntimeAsyncEventErrorV1::EngineStopped)
    ));
    drop(handle);
    let context = engine.into_context().unwrap();
    assert!(context.is_terminal());
}

#[test]
fn queued_stop_after_paired_registration_performs_no_poll_or_final_flush() {
    let (context, state, stream, event, submission) = progress_fixture();
    let config = RuntimeAsyncEngineConfigV1::default();
    let progress_config = RuntimeAsyncProgressConfigV1::default();
    let (sender, receiver) = sync_channel(config.command_capacity);
    let progress_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
    let event_cell = Arc::new(RuntimeAsyncFutureCellV1::with_progress(
        stream,
        Arc::clone(&progress_cell),
    ));
    let (response_sender, response_receiver) = sync_channel(1);
    sender
        .try_send(RuntimeAsyncEngineCommandV1::RegisterEventWithProgress {
            event,
            stream,
            event_cell: Arc::clone(&event_cell),
            progress_cell: Arc::clone(&progress_cell),
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
    assert!(progress_cell.stopped.load(Ordering::Acquire));
    let state = state.lock().unwrap();
    assert_eq!(state.poll_calls, 0);
    assert!(state.flush_calls.is_empty());
    assert_eq!(state.release_calls, 0);
    assert_eq!(
        state.statuses.get(&submission),
        Some(&BackendPollV1::Pending)
    );
    drop(state);

    let mut future = RuntimeEventFutureV1 {
        event,
        cell: event_cell,
        completed: false,
    };
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_once(&mut future, &waker),
        Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
    ));
    assert!(!context.is_terminal());
}

#[test]
fn shutdown_wakes_paired_future_only_after_progress_is_stopped() {
    let (context, _state, stream, event, _submission) = progress_fixture();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let mut future = handle.event_future_with_progress(stream, event).unwrap();
    let ordering = Arc::new(ProgressStopOrderingWake {
        progress: Arc::clone(&future.progress.cell),
        woke: AtomicBool::new(false),
        observed_stopped: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&ordering));
    assert!(poll_once(&mut future, &waker).is_pending());

    drop(handle);
    let _context = engine.into_context().unwrap();

    assert!(ordering.woke.load(Ordering::Acquire));
    assert!(ordering.observed_stopped.load(Ordering::Acquire));
    assert!(matches!(
        poll_once(&mut future, &waker),
        Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
    ));
    assert!(future.is_progress_stopped());
}
