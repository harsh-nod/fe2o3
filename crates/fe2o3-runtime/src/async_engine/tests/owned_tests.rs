use super::*;
use crate::{RuntimeAsyncOwnedDispositionV1, RuntimeOwnedShutdownBackendV1};
use std::cell::Cell;

mod control_tests;
mod drain_tests;
mod executor_tests;
mod graph_tests;
mod snapshot_tests;

#[derive(Default)]
struct OwnerTrace {
    calls: Vec<(&'static str, ThreadId)>,
    polled_submissions: Vec<u64>,
    finalizer_fails: bool,
    finalizer_panics: bool,
    cleanup_fails: bool,
    cleanup_panics: bool,
    write_panics: bool,
    flush_panics: bool,
    poll_panics: bool,
    initially_terminal: bool,
    capture_failure: Option<crate::RuntimeHostCaptureErrorV1>,
    capture_terminal: bool,
    capture_panics: bool,
    capture_backing_pending: bool,
    capture_calls: usize,
    capture_requests: Vec<(u64, u64, u64, usize)>,
}

struct ThreadBoundBackend {
    inner: MockBackend,
    owner: ThreadId,
    local: Rc<Cell<usize>>,
    trace: Arc<Mutex<OwnerTrace>>,
}

impl ThreadBoundBackend {
    fn record(&self, call: &'static str) {
        assert_eq!(thread::current().id(), self.owner);
        self.local.set(self.local.get() + 1);
        self.trace.lock().unwrap().calls.push((call, self.owner));
    }
}

macro_rules! forward {
    ($name:ident($($arg:ident: $ty:ty),*) -> $output:ty) => {
        fn $name(&mut self, $($arg: $ty),*) -> Result<$output, RuntimeBackendFailureV1<Self::Error>> {
            self.record(stringify!($name));
            self.inner.$name($($arg),*)
        }
    };
}

impl RuntimeBackendV1 for ThreadBoundBackend {
    type Error = MockError;
    forward!(enumerate_devices_v1() -> Vec<BackendDeviceDescriptionV1>);
    forward!(create_stream_v1(device: u64) -> u64);
    forward!(allocate_v1(device: u64, kind: RuntimeMemoryKindV1, byte_len: u64, alignment: u64) -> u64);
    forward!(release_allocation_v1(allocation: u64) -> ());
    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.record("write_side_effect");
        let panics = self.trace.lock().unwrap().write_panics;
        assert!(!panics, "write adapter panic after simulated side effect");
        self.inner
            .write_allocation_v1(allocation, byte_offset, bytes)
    }
    forward!(read_allocation_v1(allocation: u64, byte_offset: u64, destination: &mut [u8]) -> ());
    fn capture_coherent_host_range_v1(
        &mut self,
        mut request: crate::BackendHostCaptureV1<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<crate::RuntimeHostCaptureErrorV1>> {
        self.record("capture");
        let mut trace = self.trace.lock().unwrap();
        trace.capture_calls += 1;
        trace.capture_requests.push((
            request.device(),
            request.allocation(),
            request.byte_offset(),
            request.destination_mut().len(),
        ));
        let (failure, terminal, panics, pending) = (
            trace.capture_failure,
            trace.capture_terminal,
            trace.capture_panics,
            trace.capture_backing_pending,
        );
        drop(trace);
        if pending {
            return Err(RuntimeBackendFailureV1::Rejected(
                crate::RuntimeHostCaptureErrorV1::Pending,
            ));
        }
        if let Some(error) = failure {
            if terminal {
                request.destination_mut()[0] = 0xee;
                return Err(RuntimeBackendFailureV1::Terminal(error));
            }
            return Err(RuntimeBackendFailureV1::Rejected(error));
        }
        request.destination_mut().fill(0x5a);
        assert!(!panics, "capture adapter panic after private copy");
        Ok(())
    }
    forward!(load_module_v1(device: u64, image: &[u8]) -> u64);
    forward!(unload_module_v1(module: u64) -> ());
    forward!(resolve_kernel_v1(module: u64, name: &str, signature: [u8;32]) -> u64);
    forward!(submit_v1(launch: BackendLaunchV1<'_>) -> u64);
    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.record("poll_v1");
        let panics = {
            let mut trace = self.trace.lock().unwrap();
            trace.polled_submissions.push(submission);
            trace.poll_panics
        };
        assert!(!panics, "poll adapter panic");
        self.inner.poll_v1(submission)
    }
    forward!(wait_v1(submission: u64, deadline: Instant) -> BackendPollV1);
    forward!(release_submission_v1(submission: u64) -> ());
    forward!(record_event_v1(stream: u64, submission: u64) -> u64);
    forward!(release_event_v1(event: u64) -> ());
    forward!(peer_copy_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64);

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.record("destroy_stream_v1");
        let trace = self.trace.lock().unwrap();
        let (fails, panics) = (trace.cleanup_fails, trace.cleanup_panics);
        drop(trace);
        assert!(!panics, "scripted cleanup panic");
        if fails {
            return Err(RuntimeBackendFailureV1::Rejected(MockError("busy")));
        }
        self.inner.destroy_stream_v1(stream)
    }
}

impl RuntimeFlushBackendV1 for ThreadBoundBackend {
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.record("flush");
        let panics = self.trace.lock().unwrap().flush_panics;
        assert!(!panics, "flush adapter panic");
        self.inner.flush_stream_v1(stream)
    }
}

impl RuntimeOwnedShutdownBackendV1 for ThreadBoundBackend {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.record("finalize");
        let trace = self.trace.lock().unwrap();
        let (fails, panics) = (trace.finalizer_fails, trace.finalizer_panics);
        drop(trace);
        assert!(!panics, "scripted native finalizer panic");
        if fails {
            Err(RuntimeBackendFailureV1::Terminal(MockError(
                "native failure",
            )))
        } else {
            Ok(())
        }
    }
}

impl Drop for ThreadBoundBackend {
    fn drop(&mut self) {
        self.record("drop");
    }
}

fn start(
    state: Arc<Mutex<MockState>>,
    trace: Arc<Mutex<OwnerTrace>>,
) -> (
    RuntimeAsyncOwnedEngineV1<ThreadBoundBackend>,
    RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
) {
    start_with_config(state, trace, RuntimeAsyncEngineConfigV1::default())
}

fn start_with_config(
    state: Arc<Mutex<MockState>>,
    trace: Arc<Mutex<OwnerTrace>>,
    config: RuntimeAsyncEngineConfigV1,
) -> (
    RuntimeAsyncOwnedEngineV1<ThreadBoundBackend>,
    RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
) {
    RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || {
            let backend = ThreadBoundBackend {
                inner: MockBackend { state },
                owner: thread::current().id(),
                local: Rc::new(Cell::new(0)),
                trace,
            };
            backend.record("construct");
            let initially_terminal = backend.trace.lock().unwrap().initially_terminal;
            let mut context = RuntimeContextV1::open(backend)?;
            if initially_terminal {
                context.quarantine_after_async_command_panic_v1();
            }
            Ok::<_, RuntimeErrorV1<MockError>>(context)
        },
        config,
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap()
}

fn join_command<R>(
    future: RuntimeAsyncCommandFutureV1<R>,
) -> Result<R, RuntimeAsyncEngineCallErrorV1> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(waker)) {
            return result;
        }
        assert!(Instant::now() < deadline, "command future stalled");
        thread::yield_now();
    }
}

#[test]
fn non_send_backend_constructs_uses_finalizes_and_drops_on_one_owner() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state, Arc::clone(&trace));
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RuntimeAsyncProgressHandleV1<ThreadBoundBackend>>();
    let worker = thread::spawn(move || {
        let response = handle
            .observer()
            .enqueue_with_context(|context| {
                let stream = context.create_stream(context.devices()[0].id()).unwrap();
                (thread::current().id(), stream)
            })
            .unwrap();
        join_command(response).unwrap().0
    })
    .join()
    .unwrap();
    assert_ne!(worker, thread::current().id());
    let report = engine.shutdown().unwrap();
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(report.cleanup.unwrap().is_complete());
    assert!(!report.worker_panicked);
    assert!(report.native_failure.is_none());
    let trace = trace.lock().unwrap();
    assert!(trace.calls.iter().all(|(_, id)| *id == worker));
    assert_eq!(trace.calls.first().unwrap().0, "construct");
    assert_eq!(trace.calls[trace.calls.len() - 2].0, "finalize");
    assert_eq!(trace.calls.last().unwrap().0, "drop");
}

#[test]
fn dropped_command_future_does_not_cancel_accepted_command() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(state, Arc::new(Mutex::new(OwnerTrace::default())));
    let count = Arc::new(AtomicUsize::new(0));
    let captured = Arc::clone(&count);
    drop(
        handle
            .observer()
            .enqueue_with_context(move |_| {
                captured.fetch_add(1, AtomicOrdering::SeqCst);
            })
            .unwrap(),
    );
    engine.shutdown().unwrap();
    assert_eq!(count.load(AtomicOrdering::SeqCst), 1);
}

#[test]
fn async_command_panics_are_reported_and_owner_quarantines() {
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
    assert_eq!(
        join_command(
            handle
                .observer()
                .enqueue_with_context(|_| panic!("command panic"))
                .unwrap()
        ),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
}

#[test]
fn cleanup_and_native_failures_retain_complete_owner_without_drop() {
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
        handle
            .observer()
            .try_with_context(|context| context.create_stream(context.devices()[0].id()).unwrap())
            .unwrap();
        let report = engine.shutdown().unwrap();
        assert_eq!(
            report.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(report.worker_panicked, mode == 1 || mode == 3);
        assert_eq!(report.native_failure.is_some(), mode == 2);
        if mode == 0 {
            assert_eq!(report.cleanup.unwrap().retained().streams, 1);
        }
        assert!(
            !trace
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|(call, _)| *call == "drop")
        );
        assert!(
            !trace
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|(call, _)| *call == "finalize")
                || mode >= 2
        );
        assert!(matches!(
            handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
    }
}

#[test]
fn initializer_error_and_panic_are_reported() {
    let error = RuntimeAsyncOwnedEngineV1::<ThreadBoundBackend>::spawn_with_progress(
        || Err::<RuntimeContextV1<ThreadBoundBackend>, _>(MockError("init")),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    );
    assert!(matches!(
        error,
        Err(RuntimeAsyncOwnedSpawnErrorV1::Initialize(MockError("init")))
    ));
    let panic = RuntimeAsyncOwnedEngineV1::<ThreadBoundBackend>::spawn_with_progress(
        || -> Result<RuntimeContextV1<ThreadBoundBackend>, MockError> { panic!("init panic") },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    );
    assert!(matches!(
        panic,
        Err(RuntimeAsyncOwnedSpawnErrorV1::InitializerPanicked)
    ));
}

#[test]
fn enqueue_is_nonblocking_bounded_and_discarded_commands_resolve() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let context = RuntimeContextV1::open(MockBackend { state }).unwrap();
    let config = RuntimeAsyncEngineConfigV1::new(1, 1, 1, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let (a, b) = (Arc::clone(&entered), Arc::clone(&release));
    let first = handle
        .enqueue_with_context(move |_| {
            a.wait();
            b.wait();
        })
        .unwrap();
    entered.wait();
    let second = handle.enqueue_with_context(|_| 2).unwrap();
    assert!(matches!(
        handle.enqueue_with_context(|_| 3),
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    release.wait();
    assert_eq!(join_command(first), Ok(()));
    assert_eq!(join_command(second), Ok(2));
    let _ = engine.into_context().unwrap();

    // Exercise the receiver-drop guard without relying on thread scheduling.
    let (sender, receiver) = sync_channel(1);
    let handle = RuntimeAsyncEngineHandleV1::<MockBackend> {
        context_generation: 0,
        capture_budget: None,
        reply_budget: reply_budget::ReplyBudgetV1::new(DEFAULT_RUNTIME_ASYNC_REPLIES_V1),
        admission: drain::AdmissionV1::new(),
        graph_slot: Arc::new(AtomicBool::new(false)),
        snapshot_budget: snapshot::SnapshotBudgetV1::new(DEFAULT_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1),
        sender,
        worker_thread: Arc::new(OnceLock::new()),
        quarantine_command_panics: false,
    };
    let future = handle.enqueue_with_context(|_| 5).unwrap();
    drop(receiver);
    assert_eq!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
}

#[test]
fn reentrant_async_command_is_rejected() {
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
    let nested = handle.clone();
    let future = handle
        .observer()
        .enqueue_with_context(move |_| {
            matches!(
                nested.observer().enqueue_with_context(|_| ()),
                Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
            )
        })
        .unwrap();
    assert_eq!(join_command(future), Ok(true));
    engine.shutdown().unwrap();
}

fn launch_fixture(
    handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
) -> (
    RuntimeStreamIdV1,
    Arc<crate::TypedRuntimeKernelV1<EmptyArgs>>,
) {
    handle
        .observer()
        .try_with_context(|context| {
            let device = context.devices()[0].id();
            let stream = context.create_stream(device).unwrap();
            let module = context.load_module(device, &[1]).unwrap();
            let kernel = Arc::new(
                context
                    .resolve_kernel::<EmptyArgs>(module, "empty")
                    .unwrap(),
            );
            (stream, kernel)
        })
        .unwrap()
}

fn geometry() -> RuntimeLaunchGeometryV1 {
    RuntimeLaunchGeometryV1 {
        grid: [1, 1, 1],
        workgroup: [1, 1, 1],
        dynamic_shared_bytes: 0,
    }
}

#[test]
fn thousands_of_owned_operations_complete_out_of_order_after_observer_drop() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(
        Arc::clone(&state),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
    let (stream, kernel) = launch_fixture(&handle);
    let mut futures = Vec::new();
    for _ in 0..2048 {
        loop {
            match handle.launch(
                stream,
                Arc::clone(&kernel),
                EmptyArgs,
                geometry(),
                Vec::new(),
            ) {
                Ok(future) => {
                    futures.push(Some(future));
                    break;
                }
                Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull) => thread::yield_now(),
                Err(error) => panic!("unexpected admission error: {error}"),
            }
        }
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while state.lock().unwrap().statuses.len() != 2048 {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let mut ids: Vec<_> = state.lock().unwrap().statuses.keys().copied().collect();
    ids.sort_unstable();
    for index in (1..2048).step_by(2) {
        state
            .lock()
            .unwrap()
            .statuses
            .insert(ids[index], BackendPollV1::Succeeded);
    }
    let mut completed = HashSet::new();
    for index in (1..2048).step_by(2) {
        let result = join_command(futures[index].take().unwrap()).unwrap();
        assert_eq!(
            result.observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert!(completed.insert(result.submission.unwrap().id()));
    }
    let waker = Waker::noop();
    for index in (0..2048).step_by(2) {
        let mut future = futures[index].take().unwrap();
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(waker))
                .is_pending()
        );
        drop(future);
        state
            .lock()
            .unwrap()
            .statuses
            .insert(ids[index], BackendPollV1::Succeeded);
    }
    loop {
        let pending = handle
            .observer()
            .try_with_context(move |context| context.query_stream(stream).unwrap().pending)
            .unwrap();
        if pending == 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let state = state.lock().unwrap();
    assert_eq!(state.poll_threads.len(), 1);
    assert!(!state.flush_calls.is_empty());
    assert_eq!(state.release_calls, 0);
    drop(state);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn operation_registry_capacity_rejects_before_submission() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let context = RuntimeContextV1::open(MockBackend {
        state: Arc::clone(&state),
    })
    .unwrap();
    let config = RuntimeAsyncEngineConfigV1::new(8, 1, 8, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        config,
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let (stream, kernel) = handle
        .observer()
        .try_with_context(|context| {
            let device = context.devices()[0].id();
            let stream = context.create_stream(device).unwrap();
            let module = context.load_module(device, &[1]).unwrap();
            (
                stream,
                Arc::new(
                    context
                        .resolve_kernel::<EmptyArgs>(module, "empty")
                        .unwrap(),
                ),
            )
        })
        .unwrap();
    let first = handle
        .launch(
            stream,
            Arc::clone(&kernel),
            EmptyArgs,
            geometry(),
            Vec::new(),
        )
        .unwrap();
    wait_until(|| state.lock().unwrap().statuses.len() == 1);
    let second = handle
        .launch(stream, kernel, EmptyArgs, geometry(), Vec::new())
        .unwrap();
    assert!(matches!(
        join_command(second),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    ));
    assert_eq!(state.lock().unwrap().statuses.len(), 1);
    drop(first);
    let mut context = engine.into_context().unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn command_future_replaces_waker_and_wakes_exactly_once() {
    let (sender, receiver) = sync_channel(1);
    let handle = RuntimeAsyncEngineHandleV1::<MockBackend> {
        context_generation: 0,
        capture_budget: None,
        reply_budget: reply_budget::ReplyBudgetV1::new(DEFAULT_RUNTIME_ASYNC_REPLIES_V1),
        admission: drain::AdmissionV1::new(),
        graph_slot: Arc::new(AtomicBool::new(false)),
        snapshot_budget: snapshot::SnapshotBudgetV1::new(DEFAULT_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1),
        sender,
        worker_thread: Arc::new(OnceLock::new()),
        quarantine_command_panics: false,
    };
    let mut future = handle.enqueue_with_context(|_| 7).unwrap();
    let old = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let new = Arc::new(WakeCounter(AtomicUsize::new(0)));
    for counter in [&old, &new, &new] {
        let waker = Waker::from(Arc::clone(counter));
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    let command = receiver.recv().unwrap();
    let mut context = RuntimeContextV1::open(MockBackend {
        state: Arc::new(Mutex::new(MockState::default())),
    })
    .unwrap();
    if let RuntimeAsyncEngineCommandV1::Context(command) = command {
        command(&mut context);
    } else {
        panic!("wrong command");
    }
    assert_eq!(old.0.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(new.0.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(join_command(future), Ok(7));
}

#[test]
fn operation_poll_panic_retains_backend_and_resolves_all_futures() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(Arc::clone(&state), Arc::clone(&trace));
    let (stream, kernel) = launch_fixture(&handle);
    state.lock().unwrap().panic_on_poll = true;
    let future = handle
        .launch(stream, kernel, EmptyArgs, geometry(), Vec::new())
        .unwrap();
    assert!(matches!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    ));
    let report = engine.shutdown().unwrap();
    assert_eq!(
        report.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert!(
        !trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(call, _)| *call == "drop")
    );
}

#[test]
fn owned_operation_flush_cursor_rotates_when_full_poll_round_restores_order() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let mut context = RuntimeContextV1::open(MockBackend {
        state: Arc::clone(&state),
    })
    .unwrap();
    let device = context.devices()[0].id();
    let a = context.create_stream(device).unwrap();
    let b = context.create_stream(device).unwrap();
    struct Pending(RuntimeStreamIdV1);
    impl operation::EngineOperationV1<MockBackend> for Pending {
        fn advance(&mut self, _: &mut RuntimeContextV1<MockBackend>) -> bool {
            false
        }
        fn stream(&self) -> RuntimeStreamIdV1 {
            self.0
        }
        fn reject(&mut self, _: RuntimeAsyncEngineCallErrorV1) {}
    }
    let mut operations = operation::OperationRegistryV1::new();
    operations.insert(Box::new(Pending(a)));
    operations.insert(Box::new(Pending(b)));
    for _ in 0..4 {
        operation::advance_operations_v1(
            &mut context,
            &mut operations,
            2,
            1,
            flush_stream_v1::<MockBackend>,
        );
    }
    let state = state.lock().unwrap();
    let streams = &state.created_streams;
    assert_eq!(
        state
            .flush_calls
            .iter()
            .map(|(stream, _)| *stream)
            .collect::<Vec<_>>(),
        [streams[0], streams[1], streams[0], streams[1]]
    );
    drop(state);
    let c = context.create_stream(device).unwrap();
    let d = context.create_stream(device).unwrap();
    operations.insert(Box::new(Pending(c)));
    operations.insert(Box::new(Pending(d)));
    for _ in 0..8 {
        operation::advance_operations_v1(
            &mut context,
            &mut operations,
            2,
            1,
            flush_stream_v1::<MockBackend>,
        );
    }
    let state = context.backend().state.lock().unwrap();
    let streams = &state.created_streams;
    assert_eq!(
        state.flush_calls[4..]
            .iter()
            .map(|(stream, _)| *stream)
            .collect::<Vec<_>>(),
        [
            streams[2], streams[3], streams[0], streams[1], streams[2], streams[3], streams[0],
            streams[1]
        ]
    );
}

#[test]
fn rejected_poll_keeps_abandoned_operation_progress_without_resubmission() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(
        Arc::clone(&state),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
    let (stream, kernel) = launch_fixture(&handle);
    state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError(
            "retry observation",
        )));
    let future = handle
        .launch(stream, kernel, EmptyArgs, geometry(), Vec::new())
        .unwrap();
    drop(future);
    wait_until(|| state.lock().unwrap().poll_calls >= 3);
    assert_eq!(state.lock().unwrap().statuses.len(), 1);
    for status in state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    wait_until(|| {
        handle
            .observer()
            .try_with_context(move |context| context.query_stream(stream).unwrap().succeeded)
            .unwrap()
            == 1
    });
    assert_eq!(state.lock().unwrap().statuses.len(), 1);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn backend_side_effect_panic_through_both_command_apis_quarantines() {
    for asynchronous in [false, true] {
        let trace = Arc::new(Mutex::new(OwnerTrace {
            write_panics: true,
            ..OwnerTrace::default()
        }));
        let (engine, handle) = start(
            Arc::new(Mutex::new(MockState::default())),
            Arc::clone(&trace),
        );
        let call = |context: &mut RuntimeContextV1<ThreadBoundBackend>| {
            let allocation = context
                .allocate(
                    context.devices()[0].id(),
                    RuntimeMemoryKindV1::DeviceLocal,
                    8,
                    8,
                )
                .unwrap();
            context.write_allocation(allocation, 0, &[1; 8]).unwrap();
        };
        let result = if asynchronous {
            join_command(handle.observer().enqueue_with_context(call).unwrap())
        } else {
            handle.observer().try_with_context(call)
        };
        assert_eq!(result, Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked));
        let report = engine.shutdown().unwrap();
        assert_eq!(
            report.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(report.cleanup.unwrap().retained().allocations, 1);
        assert!(
            trace
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|(call, _)| *call == "write_side_effect")
        );
        assert!(
            !trace
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|(call, _)| *call == "finalize" || *call == "drop")
        );
    }
}

#[test]
fn flush_panic_stops_later_streams_and_retains_native_owner() {
    let trace = Arc::new(Mutex::new(OwnerTrace {
        flush_panics: true,
        ..OwnerTrace::default()
    }));
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::clone(&trace),
    );
    let (a, kernel) = launch_fixture(&handle);
    let b = handle
        .observer()
        .try_with_context(|context| context.create_stream(context.devices()[0].id()).unwrap())
        .unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let (entry, exit) = (Arc::clone(&entered), Arc::clone(&release));
    let blocker = handle
        .observer()
        .enqueue_with_context(move |_| {
            entry.wait();
            exit.wait();
        })
        .unwrap();
    entered.wait();
    let first = handle
        .launch(a, Arc::clone(&kernel), EmptyArgs, geometry(), Vec::new())
        .unwrap();
    let second = handle
        .launch(b, kernel, EmptyArgs, geometry(), Vec::new())
        .unwrap();
    release.wait();
    join_command(blocker).unwrap();
    assert!(matches!(
        join_command(first),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert!(matches!(
        join_command(second),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "flush")
            .count(),
        1
    );
    assert!(
        !trace
            .calls
            .iter()
            .any(|(call, _)| *call == "drop" || *call == "finalize")
    );
}
