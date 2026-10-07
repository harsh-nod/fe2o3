use super::*;

#[test]
fn one_background_thread_wakes_a_registered_event_future() {
    let (context, state, event, backend_submission) = fixture();
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    let mut future = handle.event_future(event).unwrap();
    let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&counter));
    assert!(poll_once(&mut future, &waker).is_pending());
    state
        .lock()
        .unwrap()
        .statuses
        .insert(backend_submission, BackendPollV1::Succeeded);
    for _ in 0..100 {
        if counter.0.load(AtomicOrdering::SeqCst) != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert_ne!(counter.0.load(AtomicOrdering::SeqCst), 0);
    assert!(matches!(
        poll_once(&mut future, &waker),
        Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
    ));
    assert_eq!(state.lock().unwrap().poll_threads.len(), 1);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn dropping_a_future_never_releases_or_cancels_runtime_work() {
    let (context, state, event, backend_submission) = fixture();
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    drop(handle.event_future(event).unwrap());
    thread::sleep(Duration::from_millis(5));
    assert_eq!(state.lock().unwrap().release_calls, 0);
    assert_eq!(
        state.lock().unwrap().statuses.get(&backend_submission),
        Some(&BackendPollV1::Pending)
    );
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn duplicate_and_over_capacity_waiters_fail_before_registration() {
    let (mut context, _state, event, _backend_submission) = fixture();
    let state = Arc::clone(&context.backend().state);
    let (second_event, _) = append_submission(&mut context, &state, 2, "second");
    let config = RuntimeAsyncEngineConfigV1::new(8, 1, 2, 2, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
    let future = handle.event_future(event).unwrap();
    assert!(matches!(
        handle.event_future(event),
        Err(RuntimeAsyncEventRegistrationErrorV1::DuplicateEvent)
    ));
    assert!(matches!(
        handle.event_future(second_event),
        Err(RuntimeAsyncEventRegistrationErrorV1::Capacity)
    ));
    drop(future);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn abandoned_waiter_immediately_frees_capacity_for_another_event() {
    let (mut context, state, event, _backend_submission) = fixture();
    let (second_event, _) = append_submission(&mut context, &state, 2, "second");
    let config = RuntimeAsyncEngineConfigV1::new(8, 1, 2, 2, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
    drop(handle.event_future(event).unwrap());
    let _second = handle.event_future(second_event).unwrap();
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn waiter_scan_obeys_its_budget_and_rotates_in_event_order() {
    let (mut context, state, first_event, _first_submission) = fixture();
    let (second_event, _) = append_submission(&mut context, &state, 2, "second");
    let mut waiters = BTreeMap::from([
        (first_event, Arc::new(RuntimeAsyncFutureCellV1::new())),
        (second_event, Arc::new(RuntimeAsyncFutureCellV1::new())),
    ]);
    let mut next_event = None;
    poll_waiters_v1(&mut context, &mut waiters, None, &mut next_event, 1);
    assert_eq!(state.lock().unwrap().poll_calls, 1);
    assert_eq!(next_event, Some(second_event));
    poll_waiters_v1(&mut context, &mut waiters, None, &mut next_event, 1);
    assert_eq!(state.lock().unwrap().poll_calls, 2);
    assert_eq!(next_event, Some(first_event));
}

#[test]
fn terminal_event_does_not_consume_pending_waiter_capacity() {
    let (mut context, state, event, _backend_submission) = fixture();
    let (second_event, second_submission) = append_submission(&mut context, &state, 2, "second");
    let config = RuntimeAsyncEngineConfigV1::new(8, 1, 2, 2, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
    let _pending = handle.event_future(event).unwrap();
    state
        .lock()
        .unwrap()
        .statuses
        .insert(second_submission, BackendPollV1::Succeeded);
    assert!(matches!(
        handle
            .try_with_context(move |context| context.poll_event(second_event))
            .unwrap(),
        Ok(RuntimeCompletionStatusV1::Succeeded)
    ));
    let mut terminal = handle.event_future(second_event).unwrap();
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_once(&mut terminal, &waker),
        Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
    ));
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn one_engine_observes_out_of_order_completions_independently() {
    let (mut context, state, first_event, first_submission) = fixture();
    let (second_event, second_submission) = append_submission(&mut context, &state, 2, "second");
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    let mut first = handle.event_future(first_event).unwrap();
    let mut second = handle.event_future(second_event).unwrap();
    let first_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let second_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let first_waker = Waker::from(Arc::clone(&first_counter));
    let second_waker = Waker::from(Arc::clone(&second_counter));
    assert!(poll_once(&mut first, &first_waker).is_pending());
    assert!(poll_once(&mut second, &second_waker).is_pending());

    state
        .lock()
        .unwrap()
        .statuses
        .insert(second_submission, BackendPollV1::Succeeded);
    for _ in 0..100 {
        if second_counter.0.load(AtomicOrdering::SeqCst) != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(
        poll_once(&mut second, &second_waker),
        Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
    ));
    assert!(poll_once(&mut first, &first_waker).is_pending());

    state
        .lock()
        .unwrap()
        .statuses
        .insert(first_submission, BackendPollV1::Failed { code: 17 });
    for _ in 0..100 {
        if first_counter.0.load(AtomicOrdering::SeqCst) != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(
        poll_once(&mut first, &first_waker),
        Poll::Ready(Ok(RuntimeCompletionStatusV1::Failed(
            crate::RuntimeCompletionFailureV1::BackendCode(17)
        )))
    ));
    assert_eq!(state.lock().unwrap().poll_threads.len(), 1);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn context_command_panic_is_contained_without_stopping_the_engine() {
    let (context, _state, event, _backend_submission) = fixture();
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    assert_eq!(
        handle.try_with_context::<(), _>(|_| { std::panic::panic_any(PanickingDropPayload) }),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    );
    assert_eq!(
        handle
            .try_with_context(move |context| context.query_event(event))
            .unwrap(),
        Ok(RuntimeCompletionStatusV1::Pending)
    );
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn context_command_reentry_is_rejected_without_deadlocking_the_engine() {
    let (context, _state, event, _backend_submission) = fixture();
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    let nested_context = handle.clone();
    assert_eq!(
        handle
            .try_with_context(move |_| nested_context.try_with_context(|_| ()))
            .unwrap(),
        Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
    );
    let nested_event = handle.clone();
    assert!(matches!(
        handle
            .try_with_context(move |_| nested_event.event_future(event))
            .unwrap(),
        Err(RuntimeAsyncEventRegistrationErrorV1::ReentrantCall)
    ));
    assert_eq!(handle.try_with_context(|_| 17).unwrap(), 17);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn panicking_executor_waker_is_contained_and_other_waiters_complete() {
    let (mut context, state, first_event, first_submission) = fixture();
    let (second_event, second_submission) = append_submission(&mut context, &state, 2, "second");
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    let mut first = handle.event_future(first_event).unwrap();
    let mut second = handle.event_future(second_event).unwrap();
    let panicking_waker = Waker::from(Arc::new(PanickingWake));
    let second_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let second_waker = Waker::from(Arc::clone(&second_counter));
    assert!(poll_once(&mut first, &panicking_waker).is_pending());
    assert!(poll_once(&mut second, &second_waker).is_pending());
    {
        let mut state = state.lock().unwrap();
        state
            .statuses
            .insert(first_submission, BackendPollV1::Succeeded);
        state
            .statuses
            .insert(second_submission, BackendPollV1::Succeeded);
    }
    for _ in 0..1000 {
        if second_counter.0.load(AtomicOrdering::SeqCst) != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert_ne!(second_counter.0.load(AtomicOrdering::SeqCst), 0);
    assert!(matches!(
        poll_until_ready(&mut first, &panicking_waker),
        Ok(RuntimeCompletionStatusV1::Succeeded)
    ));
    assert!(matches!(
        poll_until_ready(&mut second, &second_waker),
        Ok(RuntimeCompletionStatusV1::Succeeded)
    ));
    assert_eq!(handle.try_with_context(|_| 17).unwrap(), 17);
    drop(handle);
    let _context = engine.into_context().unwrap();
}

#[test]
fn worker_panic_wakes_registered_futures_as_stopped() {
    let (context, state, event, _backend_submission) = fixture();
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    let mut future = handle.event_future(event).unwrap();
    let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&counter));
    assert!(poll_once(&mut future, &waker).is_pending());
    state.lock().unwrap().panic_on_poll = true;
    for _ in 0..100 {
        if counter.0.load(AtomicOrdering::SeqCst) != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert_ne!(counter.0.load(AtomicOrdering::SeqCst), 0);
    assert!(matches!(
        poll_once(&mut future, &waker),
        Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
    ));
    drop(handle);
    assert!(matches!(
        engine.into_context(),
        Err(RuntimeAsyncEngineJoinErrorV1::WorkerPanicked)
    ));
}

#[test]
fn consuming_stop_wakes_outstanding_future_without_changing_custody() {
    let (context, state, event, backend_submission) = fixture();
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    let mut future = handle.event_future(event).unwrap();
    drop(handle);
    let _context = engine.into_context().unwrap();
    let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
    assert!(matches!(
        poll_once(&mut future, &waker),
        Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
    ));
    assert_eq!(state.lock().unwrap().release_calls, 0);
    assert_eq!(
        state.lock().unwrap().statuses.get(&backend_submission),
        Some(&BackendPollV1::Pending)
    );
}

#[test]
fn ordinary_spawn_remains_observation_only() {
    let (context, state, _stream, _event, _submission) = progress_fixture();
    let (engine, handle) =
        RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
    thread::sleep(Duration::from_millis(10));
    assert!(state.lock().unwrap().flush_calls.is_empty());
    drop(handle);
    let _context = engine.into_context().unwrap();
}
