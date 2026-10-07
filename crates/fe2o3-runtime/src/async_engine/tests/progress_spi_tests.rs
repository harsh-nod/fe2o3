use super::*;
use crate::{
    RuntimeAsyncDrainOutcomeV1, RuntimeAsyncOwnedDispositionV1, RuntimeOwnedShutdownBackendV1,
};

// This generic test backend has no native resources or separate finalizer.
impl RuntimeOwnedShutdownBackendV1 for MockBackend {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        Ok(())
    }
}

#[test]
fn progress_spi_override_does_not_change_explicit_context_flush() {
    let (mut context, state, stream, event, _) = progress_fixture();
    state.lock().unwrap().override_progress = true;
    context.progress_stream_v1(stream).unwrap();
    assert_eq!(
        context.query_event(event).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert_eq!(state.lock().unwrap().progress_calls.len(), 1);
    context.flush_stream(stream).unwrap();
    assert_eq!(state.lock().unwrap().progress_calls.len(), 1);
    assert_eq!(state.lock().unwrap().flush_calls.len(), 1);
    state.lock().unwrap().complete_on_progress = true;
    context.progress_stream_v1(stream).unwrap();
    assert_eq!(
        context.poll_event(event).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(context.cleanup().is_complete());
}

#[test]
fn progress_spi_borrowed_engine_selects_override_only_for_registered_stream() {
    let (mut context, state, stream, event, _) = progress_fixture();
    let (other, _, _) = append_submission_with_stream(&mut context, &state, 2, "other");
    state.lock().unwrap().override_progress = true;
    let backend_stream = state.lock().unwrap().created_streams[0];
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap();
    let registration = handle.register_stream(stream).unwrap();
    wait_until(|| !state.lock().unwrap().progress_calls.is_empty());
    assert_ne!(stream, other);
    assert!(
        state
            .lock()
            .unwrap()
            .progress_calls
            .iter()
            .all(|(id, _)| *id == backend_stream)
    );
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert_eq!(
        handle
            .observer()
            .try_with_context(move |context| context.query_event(event).unwrap())
            .unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    drop(registration);
    drop(handle);
    let mut context = engine.into_context().unwrap();
    state.lock().unwrap().complete_on_progress = true;
    context.progress_stream_v1(stream).unwrap();
    context.progress_stream_v1(other).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn progress_spi_owned_engine_selects_override_and_can_finish_drain() {
    let (context, state, stream, _, _) = progress_fixture();
    state.lock().unwrap().override_progress = true;
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || Ok::<_, RuntimeErrorV1<MockError>>(context),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap();
    let registration = handle.register_stream(stream).unwrap();
    wait_until(|| !state.lock().unwrap().progress_calls.is_empty());
    assert!(state.lock().unwrap().flush_calls.is_empty());
    state.lock().unwrap().complete_on_progress = true;
    let mut drain = handle.begin_drain(128).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let report = loop {
        if let Poll::Ready(result) =
            Pin::new(&mut drain).poll(&mut Context::from_waker(Waker::noop()))
        {
            break result.unwrap();
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.succeeded, 1);
    assert!(state.lock().unwrap().flush_calls.is_empty());
    drop(drain);
    drop(registration);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn progress_spi_current_thread_drain_uses_override_without_registration() {
    let (context, state, _, _, _) = progress_fixture();
    state.lock().unwrap().override_progress = true;
    state.lock().unwrap().complete_on_progress = true;
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        move || Ok::<_, RuntimeErrorV1<MockError>>(context),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap();
    let drain = handle.begin_drain(32).unwrap();
    let report = engine
        .drive_until_ready(
            std::pin::pin!(drain),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap()
        .unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.succeeded, 1);
    assert_eq!(state.lock().unwrap().progress_calls.len(), 1);
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert!(
        state
            .lock()
            .unwrap()
            .progress_calls
            .iter()
            .all(|(_, id)| *id == thread::current().id())
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn progress_spi_tracked_launch_resumes_same_future_without_explicit_registration() {
    let state = Arc::new(Mutex::new(MockState {
        override_progress: true,
        ..MockState::default()
    }));
    let mut context = RuntimeContextV1::open(MockBackend {
        state: state.clone(),
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let module = context.load_module(device, &[1]).unwrap();
    let kernel = Arc::new(
        context
            .resolve_kernel::<EmptyArgs>(module, "empty")
            .unwrap(),
    );
    let request = RuntimeAsyncLaunchRequestV1::new(
        stream,
        kernel,
        &EmptyArgs,
        RuntimeLaunchGeometryV1 {
            grid: [1, 1, 1],
            workgroup: [1, 1, 1],
            dynamic_shared_bytes: 0,
        },
        vec![],
    )
    .unwrap();
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        move || Ok::<_, RuntimeErrorV1<MockError>>(context),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap();
    let mut operation = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
    let control = operation.control();
    for _ in 0..4 {
        engine.tick().unwrap();
    }
    assert_eq!(state.lock().unwrap().issues.len(), 1);
    let before = state.lock().unwrap().progress_calls.len();
    assert!(before > 0);
    assert!(matches!(
        engine.drive_until_ready(operation.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    ));
    assert_eq!(state.lock().unwrap().progress_calls.len(), before);
    assert!(control.same_operation(&operation.control()));
    state.lock().unwrap().complete_on_progress = true;
    let result = engine
        .drive_until_ready(operation.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        result.observation.as_ref().unwrap(),
        &RuntimeCompletionStatusV1::Succeeded
    );
    assert!(result.submission.is_some());
    assert_eq!(state.lock().unwrap().issues.len(), 1);
    assert!(state.lock().unwrap().flush_calls.is_empty());
    assert!(control.same_operation(&operation.control()));
    drop(operation);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn progress_spi_engine_retains_override_error_without_falling_back_to_flush() {
    let (context, state, stream, _, _) = progress_fixture();
    {
        let mut state = state.lock().unwrap();
        state.override_progress = true;
        state
            .progress_outcomes
            .push_back(MockFlushOutcome::Rejected("progress rejected"));
    }
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap();
    let registration = handle.register_stream(stream).unwrap();
    wait_until(|| registration.failure_count() == 1);
    assert!(matches!(
        registration.take_failure(),
        Some(RuntimeErrorV1::BackendRejected(MockError(
            "progress rejected"
        )))
    ));
    assert!(state.lock().unwrap().flush_calls.is_empty());
    drop(registration);
    drop(handle);
    let mut context = engine.into_context().unwrap();
    assert!(!context.is_terminal());
    state.lock().unwrap().complete_on_progress = true;
    context.progress_stream_v1(stream).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn progress_spi_override_failures_preserve_context_error_taxonomy() {
    for outcome in [
        MockFlushOutcome::Rejected("rejected"),
        MockFlushOutcome::Quiescent("quiescent"),
        MockFlushOutcome::Terminal("terminal"),
    ] {
        let (mut context, state, stream, event, submission) = progress_fixture();
        {
            let mut state = state.lock().unwrap();
            state.override_progress = true;
            state.progress_outcomes.push_back(outcome);
        }
        let error = context.progress_stream_v1(stream).unwrap_err();
        match outcome {
            MockFlushOutcome::Rejected(_) => assert!(matches!(
                error,
                RuntimeErrorV1::BackendRejected(MockError("rejected"))
            )),
            MockFlushOutcome::Quiescent(_) => assert!(matches!(
                error,
                RuntimeErrorV1::BackendQuiescent(MockError("quiescent"))
            )),
            MockFlushOutcome::Terminal(_) => assert!(matches!(
                error,
                RuntimeErrorV1::BackendTerminal(MockError("terminal"))
            )),
            MockFlushOutcome::Success => unreachable!(),
        }
        assert_eq!(state.lock().unwrap().progress_calls.len(), 1);
        assert!(state.lock().unwrap().flush_calls.is_empty());
        assert_eq!(
            state.lock().unwrap().statuses[&submission],
            BackendPollV1::Pending
        );
        if matches!(outcome, MockFlushOutcome::Terminal(_)) {
            assert!(context.is_terminal());
            assert!(context.progress_stream_v1(stream).is_err());
            assert_eq!(state.lock().unwrap().progress_calls.len(), 1);
        } else {
            assert_eq!(
                context.query_event(event).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            state.lock().unwrap().complete_on_progress = true;
            context.progress_stream_v1(stream).unwrap();
            assert!(context.cleanup().is_complete());
        }
    }
}
