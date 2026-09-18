use super::*;
use local_operation_tests::{Mode, count, enqueue_local};
mod registration_tests;

type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<ThreadBoundBackend>;
type Handle = RuntimeAsyncProgressHandleV1<ThreadBoundBackend>;

pub(super) fn start_current(
    trace: Arc<Mutex<OwnerTrace>>,
    config: RuntimeAsyncEngineConfigV1,
) -> (Engine, Handle) {
    start_current_state(Arc::new(Mutex::new(MockState::default())), trace, config)
}

fn start_current_state(
    state: Arc<Mutex<MockState>>,
    trace: Arc<Mutex<OwnerTrace>>,
    config: RuntimeAsyncEngineConfigV1,
) -> (Engine, Handle) {
    let local = Rc::new(Cell::new(0));
    Engine::new_with_progress(
        || {
            let backend = ThreadBoundBackend {
                inner: MockBackend { state },
                owner: thread::current().id(),
                local: Rc::clone(&local),
                trace,
            };
            backend.record("construct");
            RuntimeContextV1::open(backend)
        },
        config,
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap()
}

pub(super) fn drive<F: Future>(engine: &mut Engine, future: F) -> F::Output {
    engine
        .drive_until_ready(
            std::pin::pin!(future),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap()
}

fn stream(engine: &mut Engine, handle: &Handle) -> RuntimeStreamIdV1 {
    drive(
        engine,
        handle
            .observer()
            .enqueue_with_context(|context| {
                context.create_stream(context.devices()[0].id()).unwrap()
            })
            .unwrap(),
    )
    .unwrap()
}

fn poll_once<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    Pin::new(future).poll(&mut Context::from_waker(Waker::noop()))
}

#[test]
fn current_thread_borrowed_rc_factory_and_entire_lifetime_stay_on_caller() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = start_current(trace.clone(), RuntimeAsyncEngineConfigV1::default());
    let stream = stream(&mut engine, &handle);
    let future = enqueue_local(&handle, stream, trace.clone(), Mode::Complete).unwrap();
    assert_eq!(drive(&mut engine, future), Ok(()));
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(count(&trace, "construct"), 1);
    assert_eq!(count(&trace, "local_drop"), 1);
    assert_eq!(count(&trace, "finalize"), 1);
    assert_eq!(count(&trace, "drop"), 1);
    assert!(
        trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .all(|(_, id)| *id == thread::current().id())
    );
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn current_thread_tick_preserves_command_budget_and_progress_between_calls() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(4, 4, 1, 1, Duration::from_millis(1)).unwrap();
    let (mut engine, handle) = start_current(trace, config);
    let mut first = handle.observer().enqueue_with_context(|_| 1).unwrap();
    let mut second = handle.observer().enqueue_with_context(|_| 2).unwrap();
    assert!(poll_once(&mut first).is_pending());
    assert!(poll_once(&mut second).is_pending());
    assert_eq!(engine.tick(), Ok(RuntimeAsyncTickV1::Running));
    assert_eq!(poll_once(&mut first), Poll::Ready(Ok(1)));
    assert!(poll_once(&mut second).is_pending());
    assert_eq!(engine.tick(), Ok(RuntimeAsyncTickV1::Running));
    assert_eq!(poll_once(&mut second), Poll::Ready(Ok(2)));
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_deadline_keeps_same_future_and_credit_for_later_completion() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = start_current(trace, RuntimeAsyncEngineConfigV1::default());
    let executed = Arc::new(AtomicUsize::new(0));
    let inside = executed.clone();
    let mut future = Box::pin(
        handle
            .observer()
            .enqueue_with_context(move |_| inside.fetch_add(1, Ordering::SeqCst))
            .unwrap(),
    );
    let address = &*future as *const _;
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    assert_eq!(
        engine.drive_until_ready(future.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    );
    assert_eq!(&*future as *const _, address);
    assert_eq!(executed.load(Ordering::SeqCst), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    assert_eq!(
        engine.drive_until_ready(future.as_mut(), Instant::now() + Duration::from_secs(2)),
        Ok(Ok(0))
    );
    assert_eq!(executed.load(Ordering::SeqCst), 1);
    drop(future);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_ready_result_wins_over_expired_deadline() {
    let (mut engine, _) = start_current(
        Arc::new(Mutex::new(OwnerTrace::default())),
        RuntimeAsyncEngineConfigV1::default(),
    );
    assert_eq!(
        engine.drive_until_ready(std::pin::pin!(std::future::ready(17)), Instant::now()),
        Ok(17)
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_rejects_blocking_owner_calls_and_callback_enqueue() {
    let (mut engine, handle) = start_current(
        Arc::new(Mutex::new(OwnerTrace::default())),
        RuntimeAsyncEngineConfigV1::default(),
    );
    assert_eq!(
        handle.observer().try_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
    );
    let stream = stream(&mut engine, &handle);
    assert!(matches!(
        handle.register_stream(stream),
        Err(RuntimeAsyncProgressRegistrationErrorV1::ReentrantCall)
    ));
    let nested = handle.clone();
    let future = handle
        .observer()
        .enqueue_with_context(move |_| {
            assert!(matches!(
                nested.observer().enqueue_with_context(|_| ()),
                Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
            ));
            assert!(matches!(
                nested.begin_drain(1),
                Err(RuntimeAsyncDrainErrorV1::ReentrantCall)
            ));
        })
        .unwrap();
    assert_eq!(drive(&mut engine, future), Ok(()));
    assert_eq!(
        drive(
            &mut engine,
            handle.observer().enqueue_with_context(|_| 7).unwrap()
        ),
        Ok(7)
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

struct ReentrantWake {
    handle: Handle,
    rejected: AtomicUsize,
}
impl Wake for ReentrantWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        assert!(matches!(
            self.handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
        ));
        self.rejected.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn current_thread_reply_wake_runs_under_reentrancy_guard() {
    let (mut engine, handle) = start_current(
        Arc::new(Mutex::new(OwnerTrace::default())),
        RuntimeAsyncEngineConfigV1::default(),
    );
    let mut future = handle.observer().enqueue_with_context(|_| ()).unwrap();
    let wake = Arc::new(ReentrantWake {
        handle: handle.clone(),
        rejected: AtomicUsize::new(0),
    });
    let waker = Waker::from(wake.clone());
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    engine.tick().unwrap();
    assert_eq!(wake.rejected.load(Ordering::SeqCst), 1);
    assert_eq!(poll_once(&mut future), Poll::Ready(Ok(())));
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_foreign_thread_admission_still_works() {
    let (mut engine, handle) = start_current(
        Arc::new(Mutex::new(OwnerTrace::default())),
        RuntimeAsyncEngineConfigV1::default(),
    );
    let future = thread::spawn(move || {
        handle
            .observer()
            .enqueue_with_context(|_| thread::current().id())
            .unwrap()
    })
    .join()
    .unwrap();
    assert_eq!(drive(&mut engine, future), Ok(thread::current().id()));
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_full_queue_shutdown_and_drop_do_not_execute_commands() {
    for explicit in [false, true] {
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let config = RuntimeAsyncEngineConfigV1::new(1, 1, 1, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = start_current(trace.clone(), config);
        let mut future = handle
            .observer()
            .enqueue_with_context(|_| panic!("queued Stop must not execute"))
            .unwrap();
        assert!(matches!(
            handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
        ));
        if explicit {
            assert_eq!(
                engine.shutdown().disposition,
                RuntimeAsyncOwnedDispositionV1::Released
            );
        } else {
            drop(engine);
        }
        assert!(matches!(
            poll_once(&mut future),
            Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
        ));
        drop(future);
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        assert_eq!(count(&trace, "drop"), 1);
        assert!(matches!(
            handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
    }
}

#[test]
fn current_thread_pending_driver_shutdown_obeys_native_retirement_order() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = start_current(trace.clone(), RuntimeAsyncEngineConfigV1::default());
    let stream = stream(&mut engine, &handle);
    let mut future = enqueue_local(&handle, stream, trace.clone(), Mode::Pending).unwrap();
    engine.tick().unwrap();
    assert_eq!(count(&trace, "local_first_advance"), 1);
    let report = engine.shutdown();
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(report.cleanup.unwrap().is_complete());
    assert_eq!(
        poll_once(&mut future),
        Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    );
    let trace = trace.lock().unwrap();
    let at = |name| {
        trace
            .calls
            .iter()
            .position(|(call, _)| *call == name)
            .unwrap()
    };
    assert!(at("local_reply_stopped") < at("destroy_stream_v1"));
    assert!(at("destroy_stream_v1") < at("finalize"));
    assert!(at("finalize") < at("local_drop"));
    assert!(at("local_drop") < at("drop"));
}

#[test]
fn current_thread_retains_custody_on_cleanup_and_finalizer_error_or_panic() {
    for mode in 0..4 {
        let trace = Arc::new(Mutex::new(OwnerTrace {
            cleanup_fails: mode == 0,
            cleanup_panics: mode == 1,
            finalizer_fails: mode == 2,
            finalizer_panics: mode == 3,
            ..OwnerTrace::default()
        }));
        let (mut engine, handle) =
            start_current(trace.clone(), RuntimeAsyncEngineConfigV1::default());
        let stream = stream(&mut engine, &handle);
        let mut future = enqueue_local(&handle, stream, trace.clone(), Mode::Pending).unwrap();
        engine.tick().unwrap();
        let report = engine.shutdown();
        assert_eq!(
            report.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(report.worker_panicked, mode == 1 || mode == 3);
        assert_eq!(
            poll_once(&mut future),
            Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
        );
        assert_eq!(count(&trace, "local_reply_stopped"), 1);
        assert_eq!(count(&trace, "local_drop"), 0);
        assert_eq!(count(&trace, "drop"), 0);
        assert_eq!(count(&trace, "finalize"), usize::from(mode >= 2));
    }
}

#[test]
fn current_thread_drain_finishes_accepted_prefix_across_ticks() {
    let config = RuntimeAsyncEngineConfigV1::new(4, 4, 1, 1, Duration::from_millis(1)).unwrap();
    let (mut engine, handle) = start_current(Arc::new(Mutex::new(OwnerTrace::default())), config);
    let mut a = handle.observer().enqueue_with_context(|_| 1).unwrap();
    let mut b = handle.observer().enqueue_with_context(|_| 2).unwrap();
    let drain = handle.begin_drain(16).unwrap();
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    let report = drive(&mut engine, drain).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert!(report.queued_commands_exhausted);
    assert_eq!(poll_once(&mut a), Poll::Ready(Ok(1)));
    assert_eq!(poll_once(&mut b), Poll::Ready(Ok(2)));
    assert_eq!(engine.tick(), Ok(RuntimeAsyncTickV1::Stopped));
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_future_poll_can_enqueue_between_ticks() {
    let (mut engine, handle) = start_current(
        Arc::new(Mutex::new(OwnerTrace::default())),
        RuntimeAsyncEngineConfigV1::default(),
    );
    assert_eq!(
        drive(&mut engine, async {
            handle
                .observer()
                .enqueue_with_context(|_| 23)
                .unwrap()
                .await
        }),
        Ok(23)
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_submitted_deadline_preserves_original_control_and_submission() {
    use crate::RuntimeAsyncOperationPhaseV1 as Phase;
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = start_current_state(
        state.clone(),
        trace.clone(),
        RuntimeAsyncEngineConfigV1::default(),
    );
    let request = drive(
        &mut engine,
        handle
            .observer()
            .enqueue_with_context(|context| {
                let device = context.devices()[0].id();
                let stream = context.create_stream(device).unwrap();
                let module = context.load_module(device, &[1]).unwrap();
                let kernel = Arc::new(
                    context
                        .resolve_kernel::<EmptyArgs>(module, "empty")
                        .unwrap(),
                );
                RuntimeAsyncLaunchRequestV1::new(stream, kernel, &EmptyArgs, geometry(), vec![])
                    .unwrap()
            })
            .unwrap(),
    )
    .unwrap();
    let mut future = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
    let control = future.control();
    engine.tick().unwrap();
    assert_eq!(state.lock().unwrap().statuses.len(), 1);
    let credits = handle.observer().reply_cells_in_use();
    assert!(matches!(
        engine.drive_until_ready(future.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    ));
    assert!(control.same_operation(&future.control()));
    assert_eq!(control.phase(), Phase::Observing);
    assert_eq!(handle.observer().reply_cells_in_use(), credits);
    assert!(trace.lock().unwrap().released_submissions.is_empty());
    for status in state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    let result = engine
        .drive_until_ready(future.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(result.submission.is_some());
    assert!(control.same_operation(&future.control()));
    assert_eq!(control.phase(), Phase::ObservationFinished);
    assert_eq!(state.lock().unwrap().issues.len(), 1);
    drop(future);
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn current_thread_graph_progress_and_retirement_survive_separate_ticks() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(4, 4, 1, 1, Duration::from_millis(1)).unwrap();
    let (mut engine, handle) = start_current_state(state.clone(), trace.clone(), config);
    let request = drive(
        &mut engine,
        handle
            .observer()
            .enqueue_with_context(graph_tests::owner_request)
            .unwrap(),
    )
    .unwrap();
    let mut future = Box::pin(handle.submit_graph(request).unwrap());
    let mut report = None;
    for _ in 0..16 {
        engine.tick().unwrap();
        for status in state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
        if let Poll::Ready(value) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            report = Some(value.unwrap().unwrap());
            break;
        }
    }
    assert_eq!(
        report
            .expect("graph did not complete in bounded ticks")
            .observations
            .len(),
        1
    );
    assert_eq!(count(&trace, "submit_v1"), 1);
    assert_eq!(count(&trace, "release_submission_v1"), 1);
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn current_thread_unit_poll_budget_advances_both_streams() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(4, 4, 1, 1, Duration::from_millis(1)).unwrap();
    let (mut engine, handle) = start_current(trace.clone(), config);
    let a = stream(&mut engine, &handle);
    let b = stream(&mut engine, &handle);
    let mut first = enqueue_local(&handle, a, trace.clone(), Mode::Pending).unwrap();
    let mut second = enqueue_local(&handle, b, trace.clone(), Mode::Pending).unwrap();
    for _ in 0..4 {
        engine.tick().unwrap();
    }
    assert_eq!(count(&trace, "local_first_advance"), 2);
    assert!(poll_once(&mut first).is_pending());
    assert!(poll_once(&mut second).is_pending());
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(count(&trace, "local_drop"), 2);
}

#[test]
fn current_thread_exhausted_drain_retains_pending_driver() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = start_current(trace.clone(), RuntimeAsyncEngineConfigV1::default());
    let stream = stream(&mut engine, &handle);
    let mut future = enqueue_local(&handle, stream, trace.clone(), Mode::Pending).unwrap();
    engine.tick().unwrap();
    let report = drive(&mut engine, handle.begin_drain(1).unwrap()).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::BudgetExhausted);
    assert_eq!(report.operations_remaining, 1);
    assert_eq!(
        poll_once(&mut future),
        Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(count(&trace, "drop"), 0);
}

#[test]
fn current_thread_outer_panic_retains_custody_and_guards_recovery_wake() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = start_current(trace.clone(), RuntimeAsyncEngineConfigV1::default());
    let stream = stream(&mut engine, &handle);
    let mut future = enqueue_local(&handle, stream, trace.clone(), Mode::Pending).unwrap();
    engine.tick().unwrap();
    let wake = Arc::new(ReentrantWake {
        handle: handle.clone(),
        rejected: AtomicUsize::new(0),
    });
    let waker = Waker::from(wake.clone());
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    assert!(
        handle
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::Context(Box::new(|_| panic!(
                "uncontained test callback"
            ))))
            .is_ok()
    );
    assert_eq!(engine.tick(), Err(RuntimeAsyncDriveErrorV1::OwnerPanicked));
    assert_eq!(wake.rejected.load(Ordering::SeqCst), 1);
    assert_eq!(
        poll_once(&mut future),
        Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    );
    assert_eq!(engine.tick(), Ok(RuntimeAsyncTickV1::Stopped));
    assert_eq!(
        engine.drive_until_ready(
            std::pin::pin!(std::future::pending::<()>()),
            Instant::now() + Duration::from_secs(2)
        ),
        Err(RuntimeAsyncDriveErrorV1::EngineStopped)
    );
    let report = engine.shutdown();
    assert!(report.worker_panicked);
    assert_eq!(
        report.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(count(&trace, "drop"), 0);
}

#[test]
fn current_thread_invalid_configuration_does_not_call_factory() {
    let calls = Rc::new(Cell::new(0));
    let config = RuntimeAsyncEngineConfigV1 {
        command_capacity: 0,
        ..RuntimeAsyncEngineConfigV1::default()
    };
    let result = Engine::new_with_progress(
        || {
            calls.set(calls.get() + 1);
            Err::<RuntimeContextV1<ThreadBoundBackend>, _>(())
        },
        config,
        RuntimeAsyncProgressConfigV1::default(),
    );
    assert!(matches!(
        result,
        Err(RuntimeAsyncCurrentThreadInitErrorV1::InvalidEngineConfig(_))
    ));
    assert_eq!(calls.get(), 0);
    let config = RuntimeAsyncProgressConfigV1 {
        stream_capacity: 0,
        ..RuntimeAsyncProgressConfigV1::default()
    };
    let result = Engine::new_with_progress(
        || {
            calls.set(calls.get() + 1);
            Err::<RuntimeContextV1<ThreadBoundBackend>, _>(())
        },
        RuntimeAsyncEngineConfigV1::default(),
        config,
    );
    assert!(matches!(
        result,
        Err(RuntimeAsyncCurrentThreadInitErrorV1::InvalidProgressConfig(
            _
        ))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn current_thread_factory_error_and_panic_are_reported() {
    let result = Engine::new_with_progress(
        || Err::<RuntimeContextV1<ThreadBoundBackend>, _>(17),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    );
    assert!(matches!(
        result,
        Err(RuntimeAsyncCurrentThreadInitErrorV1::Initialize(17))
    ));
    let result = Engine::new_with_progress(
        || -> Result<RuntimeContextV1<ThreadBoundBackend>, ()> { panic!("factory panic") },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    );
    assert!(matches!(
        result,
        Err(RuntimeAsyncCurrentThreadInitErrorV1::InitializerPanicked)
    ));
}
