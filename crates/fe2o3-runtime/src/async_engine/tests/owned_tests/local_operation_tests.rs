use super::*;
use operation::{EngineOperationFactoryV1, EngineOperationV1};

#[derive(Clone, Copy)]
enum Mode {
    Pending,
    Complete,
    FactoryPanic,
    AdvancePanic,
    TerminalRetirement,
    FactoryRejectPanic,
    DriverRejectPanic,
}

struct LocalFactory {
    stream: RuntimeStreamIdV1,
    trace: Arc<Mutex<OwnerTrace>>,
    reply: Option<owned::Reply<()>>,
    mode: Mode,
}

fn record(trace: &Mutex<OwnerTrace>, name: &'static str) {
    trace
        .lock()
        .unwrap()
        .calls
        .push((name, thread::current().id()));
}

fn count(trace: &Mutex<OwnerTrace>, name: &'static str) -> usize {
    trace
        .lock()
        .unwrap()
        .calls
        .iter()
        .filter(|(call, _)| *call == name)
        .count()
}

impl<B: RuntimeBackendV1 + 'static> EngineOperationFactoryV1<B> for LocalFactory {
    fn materialize(&mut self) -> Box<dyn EngineOperationV1<B>> {
        record(&self.trace, "local_materialize");
        assert!(
            !matches!(self.mode, Mode::FactoryPanic),
            "inert factory panic"
        );
        let local = Rc::new(Cell::new(0));
        Box::new(LocalDriver {
            stream: self.stream,
            trace: Arc::clone(&self.trace),
            reply: self.reply.take(),
            mode: self.mode,
            owner: thread::current().id(),
            local,
            stream_calls: Cell::new(0),
        })
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        if let Some(mut reply) = self.reply.take() {
            reply.complete(Err(error));
            assert!(
                !matches!(self.mode, Mode::FactoryRejectPanic),
                "factory rejection panic after reply"
            );
        }
    }
}

struct LocalDriver {
    stream: RuntimeStreamIdV1,
    trace: Arc<Mutex<OwnerTrace>>,
    reply: Option<owned::Reply<()>>,
    mode: Mode,
    owner: ThreadId,
    local: Rc<Cell<usize>>,
    stream_calls: Cell<usize>,
}

impl<B: RuntimeBackendV1> EngineOperationV1<B> for LocalDriver {
    fn advance(&mut self, context: &mut RuntimeContextV1<B>) -> bool {
        assert_eq!(thread::current().id(), self.owner);
        if self.local.get() == 0 {
            record(&self.trace, "local_first_advance");
        }
        self.local.set(self.local.get() + 1);
        match self.mode {
            Mode::Pending | Mode::FactoryRejectPanic | Mode::DriverRejectPanic => false,
            Mode::Complete | Mode::TerminalRetirement => {
                if matches!(self.mode, Mode::TerminalRetirement) {
                    context.quarantine_after_async_command_panic_v1();
                }
                self.reply.take().unwrap().complete(Ok(()));
                true
            }
            Mode::AdvancePanic => panic!("driver panic after simulated effect"),
            Mode::FactoryPanic => unreachable!(),
        }
    }

    fn stream(&self) -> Option<RuntimeStreamIdV1> {
        assert_eq!(self.stream_calls.replace(1), 0, "stream must be cached");
        Some(self.stream)
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        assert_eq!(thread::current().id(), self.owner);
        if let Some(mut reply) = self.reply.take() {
            record(&self.trace, "local_reply_stopped");
            reply.complete(Err(error));
            assert!(
                !matches!(self.mode, Mode::DriverRejectPanic),
                "driver rejection panic after reply"
            );
        }
    }
}

impl Drop for LocalDriver {
    fn drop(&mut self) {
        assert_eq!(thread::current().id(), self.owner);
        record(&self.trace, "local_drop");
    }
}

fn enqueue_local<B: RuntimeBackendV1 + 'static>(
    handle: &RuntimeAsyncProgressHandleV1<B>,
    stream: RuntimeStreamIdV1,
    trace: Arc<Mutex<OwnerTrace>>,
    mode: Mode,
) -> Result<RuntimeAsyncCommandFutureV1<()>, RuntimeAsyncEngineCallErrorV1> {
    let (reply, future) = owned::Reply::budgeted_pair(&handle.observer.reply_budget)?;
    let factory = LocalFactory {
        stream,
        trace,
        reply: Some(reply),
        mode,
    };
    match handle
        .observer
        .try_send_command(RuntimeAsyncEngineCommandV1::Operation(Box::new(factory)))
    {
        Ok(()) => Ok(future),
        Err(TrySendError::Full(_)) => Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull),
        Err(TrySendError::Disconnected(_)) => Err(RuntimeAsyncEngineCallErrorV1::EngineStopped),
    }
}

fn stream<B: RuntimeBackendV1 + 'static>(
    handle: &RuntimeAsyncProgressHandleV1<B>,
) -> RuntimeStreamIdV1 {
    handle
        .observer()
        .try_with_context(|context| context.create_stream(context.devices()[0].id()).unwrap())
        .unwrap()
}

#[test]
fn local_driver_constructs_advances_and_retires_on_owner_thread() {
    fn assert_send<T: Send>() {}
    assert_send::<RuntimeAsyncEngineCommandV1<ThreadBoundBackend>>();
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let future =
        enqueue_local(&handle, stream(&handle), Arc::clone(&trace), Mode::Complete).unwrap();
    assert_eq!(join_command(future), Ok(()));
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(count(&trace, "local_materialize"), 1);
    assert_eq!(count(&trace, "local_first_advance"), 1);
    assert_eq!(count(&trace, "local_drop"), 1);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let trace = trace.lock().unwrap();
    let owner = trace.calls[0].1;
    assert_ne!(owner, thread::current().id());
    assert!(trace.calls.iter().all(|(_, thread)| *thread == owner));
}

#[test]
fn local_pending_driver_outlives_cleanup_and_native_shutdown() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let future =
        enqueue_local(&handle, stream(&handle), Arc::clone(&trace), Mode::Pending).unwrap();
    wait_until(|| count(&trace, "local_first_advance") == 1);
    let report = engine.shutdown().unwrap();
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(report.cleanup.unwrap().is_complete());
    assert_eq!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let calls: Vec<_> = trace
        .lock()
        .unwrap()
        .calls
        .iter()
        .map(|(name, _)| *name)
        .collect();
    let at = |name| calls.iter().position(|call| *call == name).unwrap();
    assert!(at("local_reply_stopped") < at("destroy_stream_v1"));
    assert!(at("destroy_stream_v1") < at("finalize"));
    assert!(at("finalize") < at("local_drop"));
    assert!(at("local_drop") < at("drop"));
}

#[test]
fn local_custody_survives_every_cleanup_and_native_shutdown_failure() {
    for mode in 0..4 {
        let trace = Arc::new(Mutex::new(OwnerTrace {
            cleanup_fails: mode == 0,
            cleanup_panics: mode == 1,
            finalizer_fails: mode == 2,
            finalizer_panics: mode == 3,
            ..OwnerTrace::default()
        }));
        let (engine, handle) = start(
            Arc::new(Mutex::new(MockState::default())),
            Arc::clone(&trace),
        );
        let future =
            enqueue_local(&handle, stream(&handle), Arc::clone(&trace), Mode::Pending).unwrap();
        wait_until(|| count(&trace, "local_first_advance") == 1);
        let report = engine.shutdown().unwrap();
        assert_eq!(
            report.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(report.worker_panicked, mode == 1 || mode == 3);
        assert_eq!(
            join_command(future),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        );
        assert_eq!(count(&trace, "local_reply_stopped"), 1);
        assert_eq!(count(&trace, "local_drop"), 0);
        assert_eq!(count(&trace, "drop"), 0);
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
    }
}

#[test]
fn local_advance_panic_retains_installed_driver_without_retry() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let future = enqueue_local(
        &handle,
        stream(&handle),
        Arc::clone(&trace),
        Mode::AdvancePanic,
    )
    .unwrap();
    assert_eq!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(count(&trace, "local_first_advance"), 1);
    assert_eq!(count(&trace, "local_reply_stopped"), 1);
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(count(&trace, "drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn local_custody_survives_outer_owner_loop_unwind() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let future =
        enqueue_local(&handle, stream(&handle), Arc::clone(&trace), Mode::Pending).unwrap();
    wait_until(|| count(&trace, "local_first_advance") == 1);
    assert!(
        handle
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::Context(Box::new(|_| panic!(
                "uncontained test-only owner callback"
            )),))
            .is_ok()
    );
    assert_eq!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    let report = engine.shutdown().unwrap();
    assert!(report.worker_panicked);
    assert_eq!(
        report.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(count(&trace, "drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn terminal_context_cannot_promote_driver_retirement() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let future = enqueue_local(
        &handle,
        stream(&handle),
        Arc::clone(&trace),
        Mode::TerminalRetirement,
    )
    .unwrap();
    assert_eq!(join_command(future), Ok(()));
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(count(&trace, "drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn inert_factory_panic_resolves_unique_reply_before_any_advance() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let future = enqueue_local(
        &handle,
        stream(&handle),
        Arc::clone(&trace),
        Mode::FactoryPanic,
    )
    .unwrap();
    assert_eq!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(count(&trace, "local_materialize"), 1);
    assert_eq!(count(&trace, "local_first_advance"), 0);
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn local_factory_is_rejected_by_context_returning_engine_before_materialization() {
    let context = RuntimeContextV1::open(MockBackend {
        state: Arc::new(Mutex::new(MockState::default())),
    })
    .unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let stream = stream(&handle);
    let future = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
    assert_eq!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert_eq!(count(&trace, "local_materialize"), 0);
    let mut context = engine.into_context().unwrap();
    assert!(context.query_stream(stream).is_ok());
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&context);
    assert!(context.cleanup().is_complete());
}

#[test]
fn local_and_ordinary_operations_share_capacity_before_materialization() {
    for local_first in [false, true] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let config = RuntimeAsyncEngineConfigV1::new(8, 1, 8, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = start_with_config(Arc::clone(&state), Arc::clone(&trace), config);
        let (stream, kernel) = launch_fixture(&handle);
        if local_first {
            let first = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
            wait_until(|| count(&trace, "local_first_advance") == 1);
            let second = handle
                .launch(stream, kernel, EmptyArgs, geometry(), Vec::new())
                .unwrap();
            assert!(matches!(
                join_command(second),
                Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
            ));
            let third = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
            assert_eq!(
                join_command(third),
                Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
            );
            assert_eq!(count(&trace, "local_materialize"), 1);
            assert!(state.lock().unwrap().statuses.is_empty());
            drop(first);
        } else {
            let first = handle
                .launch(stream, kernel, EmptyArgs, geometry(), Vec::new())
                .unwrap();
            wait_until(|| state.lock().unwrap().statuses.len() == 1);
            let second = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
            assert_eq!(
                join_command(second),
                Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
            );
            assert_eq!(count(&trace, "local_materialize"), 0);
            drop(first);
        }
        assert_eq!(
            engine.shutdown().unwrap().disposition,
            RuntimeAsyncOwnedDispositionV1::Released
        );
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
    }
}

#[test]
fn dropping_local_observer_does_not_dispose_or_cancel_installed_driver() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let future =
        enqueue_local(&handle, stream(&handle), Arc::clone(&trace), Mode::Pending).unwrap();
    drop(future);
    wait_until(|| count(&trace, "local_first_advance") == 1);
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(count(&trace, "local_drop"), 1);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn local_retirement_reclaims_capacity_and_stops_flushing_retired_stream() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(8, 1, 8, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(Arc::clone(&state), Arc::clone(&trace), config);
    let local_stream = stream(&handle);
    let first = enqueue_local(&handle, local_stream, Arc::clone(&trace), Mode::Complete).unwrap();
    assert_eq!(join_command(first), Ok(()));
    let (stream, kernel) = launch_fixture(&handle);
    state.lock().unwrap().complete_on_flush = true;
    let second = handle
        .launch(stream, kernel, EmptyArgs, geometry(), Vec::new())
        .unwrap();
    assert_eq!(
        join_command(second).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    let state = state.lock().unwrap();
    assert_eq!(state.created_streams.len(), 2);
    assert!(!state.flush_calls.is_empty());
    assert!(
        state
            .flush_calls
            .iter()
            .all(|(id, _)| *id == state.created_streams[1])
    );
    assert_eq!(count(&trace, "local_drop"), 1);
}

#[test]
fn local_reply_exhaustion_rejects_before_factory_materialization() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::default()
        .with_reply_capacity(1)
        .unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
        config,
    );
    let stream = stream(&handle);
    let first = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
    wait_until(|| count(&trace, "local_first_advance") == 1);
    assert!(matches!(
        enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending),
        Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    assert_eq!(count(&trace, "local_materialize"), 1);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(
        join_command(first),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn local_queue_and_closed_admission_reject_before_materialization() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(1, 1, 1, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
        config,
    );
    let stream = stream(&handle);
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let paused = handle
        .observer()
        .enqueue_with_context(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        })
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let first = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
    assert!(matches!(
        enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending),
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    assert_eq!(count(&trace, "local_materialize"), 0);
    release_tx.send(()).unwrap();
    join_command(paused).unwrap();
    wait_until(|| count(&trace, "local_first_advance") == 1);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(
        join_command(first),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert!(matches!(
        enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(count(&trace, "local_materialize"), 1);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn quarantined_local_driver_wakes_only_the_latest_registered_observer_once() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let stream = stream(&handle);
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let paused = handle
        .observer()
        .enqueue_with_context(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        })
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut future =
        enqueue_local(&handle, stream, Arc::clone(&trace), Mode::AdvancePanic).unwrap();
    let old = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let latest = Arc::new(WakeCounter(AtomicUsize::new(0)));
    for counter in [&old, &latest, &latest] {
        let waker = Waker::from(Arc::clone(counter));
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    release_tx.send(()).unwrap();
    join_command(paused).unwrap();
    wait_until(|| latest.0.load(AtomicOrdering::SeqCst) == 1);
    assert_eq!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(old.0.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(latest.0.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn factory_rejection_panic_keeps_original_reply_and_retains_installed_sibling() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(8, 1, 8, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
        config,
    );
    let stream = stream(&handle);
    let first = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
    wait_until(|| count(&trace, "local_first_advance") == 1);
    let second = enqueue_local(
        &handle,
        stream,
        Arc::clone(&trace),
        Mode::FactoryRejectPanic,
    )
    .unwrap();
    assert_eq!(
        join_command(second),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    );
    assert_eq!(
        join_command(first),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(count(&trace, "local_materialize"), 1);
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn driver_rejection_panic_retains_roster_but_still_stops_other_replies() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let stream = stream(&handle);
    let first =
        enqueue_local(&handle, stream, Arc::clone(&trace), Mode::DriverRejectPanic).unwrap();
    let second = enqueue_local(&handle, stream, Arc::clone(&trace), Mode::Pending).unwrap();
    wait_until(|| count(&trace, "local_first_advance") == 2);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(
        join_command(first),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert_eq!(
        join_command(second),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert_eq!(count(&trace, "local_reply_stopped"), 2);
    assert_eq!(count(&trace, "local_drop"), 0);
    assert_eq!(count(&trace, "drop"), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}
