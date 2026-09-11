use super::*;
use crate::{
    RuntimeAsyncCancelResultV1 as Cancel, RuntimeAsyncOperationPhaseV1 as Phase,
    RuntimeAsyncTimeoutResultV1 as Timeout, RuntimeAsyncTrackedOperationV1,
};
use std::future::{pending, ready};
use std::marker::PhantomPinned;

impl crate::RuntimeAsyncCopyBackendV1 for MockBackend {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        if let Some(error) = self.state.lock().unwrap().submit_failures.pop_front() {
            return Err(error);
        }
        let id = self.next();
        self.state.lock().unwrap().copy_issues.push((
            stream,
            source,
            destination,
            dependencies.to_vec(),
        ));
        self.state
            .lock()
            .unwrap()
            .issues
            .push((stream, id, Vec::new(), Vec::new()));
        self.state
            .lock()
            .unwrap()
            .statuses
            .insert(id, BackendPollV1::Pending);
        Ok(id)
    }
}

#[test]
fn r62_copy_and_peer_use_existing_admission_and_cancellation() {
    let mut h = Harness::new();
    let device = h.context.devices()[0].id();
    let source = h
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let destination = h
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let source = crate::RuntimeMemoryRegionV1 {
        allocation: source,
        access: crate::RuntimeAccessV1::Read,
        byte_offset: 0,
        byte_len: 64,
    };
    let destination = crate::RuntimeMemoryRegionV1 {
        allocation: destination,
        access: crate::RuntimeAccessV1::Write,
        byte_offset: 0,
        byte_len: 64,
    };
    let copy = h
        .handle
        .copy_async_tracked(h.stream, source, destination, Vec::new())
        .unwrap();
    let peer = h
        .handle
        .peer_copy_tracked(h.stream, source, destination, Vec::new())
        .unwrap();
    assert_eq!(
        copy.control().cancel_before_submission(),
        Cancel::CancelledBeforeSubmission
    );
    assert_eq!(
        peer.control().cancel_before_submission(),
        Cancel::CancelledBeforeSubmission
    );
    assert!(h.receive().advance(&mut h.context));
    assert!(h.receive().advance(&mut h.context));
    assert!(matches!(
        join(copy),
        Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
    ));
    assert!(matches!(
        join(peer),
        Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
    ));
    assert!(h.state.lock().unwrap().statuses.is_empty());
    let copy = h
        .handle
        .copy_async_tracked(h.stream, source, destination, Vec::new())
        .unwrap();
    let mut operation = h.receive();
    assert!(!operation.advance(&mut h.context));
    h.succeed();
    assert!(operation.advance(&mut h.context));
    assert_eq!(
        join(copy).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    let peer = h
        .handle
        .peer_copy_tracked(h.stream, source, destination, Vec::new())
        .unwrap();
    let control = peer.control();
    assert!(h.receive().advance(&mut h.context));
    let rejection = join(peer).unwrap();
    assert!(rejection.submission.is_none());
    assert!(rejection.observation.is_err());
    assert_eq!(control.phase(), Phase::ObservationFinished);
    assert_eq!(h.state.lock().unwrap().statuses.len(), 1);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r62_completion_inside_timer_poll_preserves_ready_result() {
    let mut h = Harness::new();
    let future = h.launch();
    let control = future.control();
    let mut operation = h.receive();
    assert!(!operation.advance(&mut h.context));
    h.succeed();
    let timer = std::future::poll_fn(|_| {
        assert!(operation.advance(&mut h.context));
        Poll::Ready(())
    });
    let future = match join(future.observe_with_timeout(timer)) {
        Timeout::TimedOut { operation } => operation,
        _ => panic!("first operation poll must be pending"),
    };
    assert!(control.same_operation(&future.control()));
    assert_eq!(control.phase(), Phase::ObservationFinished);
    assert!(join(future).unwrap().submission.is_some());
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r62_cancel_survives_capacity_rejection_and_channel_drop() {
    for reject in [false, true] {
        let h = Harness::new();
        let future = h.launch();
        let control = future.control();
        assert_eq!(
            control.cancel_before_submission(),
            Cancel::CancelledBeforeSubmission
        );
        if reject {
            h.receive_factory()
                .reject(RuntimeAsyncEngineCallErrorV1::OperationCapacity);
        }
        drop(h);
        assert!(matches!(
            join(future),
            Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
        ));
        assert_eq!(control.phase(), Phase::CancelledBeforeSubmission);
    }
}

#[test]
fn r62_rejected_poll_keeps_exact_submission_and_closed_cancellation() {
    let mut h = Harness::new();
    let future = h.launch();
    let control = future.control();
    let mut operation = h.receive();
    assert!(!operation.advance(&mut h.context));
    h.state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError("transient")));
    assert!(!operation.advance(&mut h.context));
    assert_eq!(control.phase(), Phase::Observing);
    assert_eq!(
        control.cancel_before_submission(),
        Cancel::NotCancellable(Phase::Observing)
    );
    h.succeed();
    assert!(operation.advance(&mut h.context));
    let result = join(future).unwrap();
    assert_eq!(result.rejected_observations, 1);
    assert_eq!(
        result.last_rejected_observation,
        Some(MockError("transient"))
    );
    assert_eq!(h.state.lock().unwrap().statuses.len(), 1);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r62_waker_disposal_can_reenter_reply_and_panic_without_poisoning() {
    struct DropWake(Option<Box<dyn FnOnce() + Send + Sync>>);
    impl Wake for DropWake {
        fn wake(self: Arc<Self>) {}
    }
    impl Drop for DropWake {
        fn drop(&mut self) {
            self.0.take().unwrap()();
        }
    }
    for replace in [false, true] {
        for panics in [false, true] {
            let (reply, mut future) = owned::Reply::<u64>::pair();
            let reply = Arc::new(Mutex::new(reply));
            let callback_reply = Arc::clone(&reply);
            let waker = Waker::from(Arc::new(DropWake(Some(Box::new(move || {
                callback_reply.lock().unwrap().complete(Ok(42));
                assert!(!panics, "scripted executor waker Drop panic");
            })))));
            assert!(poll(&mut future, &waker).is_pending());
            drop(waker);
            if replace {
                assert!(poll(&mut future, Waker::noop()).is_pending());
            } else {
                future.clear_waker();
            }
            assert_eq!(join(future), Ok(42));
        }
    }
    let (reply, mut future) = owned::Reply::<u64>::pair();
    let waker = Waker::from(Arc::new(DropWake(Some(Box::new(|| panic!("Drop"))))));
    assert!(poll(&mut future, &waker).is_pending());
    drop(waker);
    drop(future);
    drop(reply);
}

#[test]
fn r62_thousands_of_tracked_operations_mix_timeout_cancel_and_drop() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(
        Arc::clone(&state),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
    let (stream, kernel) = launch_fixture(&handle);
    let streams = handle
        .observer()
        .try_with_context(move |context| {
            let device = context.devices()[0].id();
            [
                stream,
                context.create_stream(device).unwrap(),
                context.create_stream(device).unwrap(),
                context.create_stream(device).unwrap(),
            ]
        })
        .unwrap();
    let mut observers = Vec::new();
    let mut controls = Vec::new();
    let mut cancelled = 0;
    for index in 0..2048 {
        let future = loop {
            match handle.launch_tracked(
                streams[index % 4],
                Arc::clone(&kernel),
                EmptyArgs,
                geometry(),
                Vec::new(),
            ) {
                Ok(future) => break future,
                Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull) => thread::yield_now(),
                Err(error) => panic!("unexpected admission failure: {error}"),
            }
        };
        let control = future.control();
        assert!(controls.iter().all(|prior| !control.same_operation(prior)));
        if index % 4 == 0 && control.cancel_before_submission() == Cancel::CancelledBeforeSubmission
        {
            cancelled += 1;
        }
        let future = if index % 4 == 1 {
            match join(future.observe_with_timeout(ready(()))) {
                Timeout::TimedOut { operation } => operation,
                _ => panic!("no submission has completed"),
            }
        } else {
            future
        };
        controls.push(control);
        observers.push(if index % 4 == 2 {
            drop(future);
            None
        } else {
            Some(future)
        });
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while state.lock().unwrap().statuses.len() != 2048 - cancelled {
        assert!(
            Instant::now() < deadline,
            "owner did not admit expected operations"
        );
        thread::sleep(Duration::from_millis(1));
    }
    let mut ids: Vec<_> = state.lock().unwrap().statuses.keys().copied().collect();
    ids.sort_unstable();
    for id in ids.into_iter().rev() {
        state
            .lock()
            .unwrap()
            .statuses
            .insert(id, BackendPollV1::Succeeded);
    }
    let mut submissions = HashSet::new();
    for (control, future) in controls.iter().zip(observers) {
        if let Some(future) = future {
            match join(future) {
                Ok(result) => {
                    assert_eq!(
                        result.observation.unwrap(),
                        RuntimeCompletionStatusV1::Succeeded
                    );
                    assert!(submissions.insert(result.submission.unwrap().id()));
                    assert_eq!(control.phase(), Phase::ObservationFinished);
                }
                Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission) => {
                    assert_eq!(control.phase(), Phase::CancelledBeforeSubmission);
                }
                Err(error) => panic!("unexpected result: {error}"),
            }
        }
    }
    wait_until(|| {
        controls.iter().all(|c| {
            matches!(
                c.phase(),
                Phase::ObservationFinished | Phase::CancelledBeforeSubmission
            )
        })
    });
    assert_eq!(state.lock().unwrap().poll_threads.len(), 1);
    assert_eq!(state.lock().unwrap().release_calls, 0);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(state.lock().unwrap().release_calls, 2048 - cancelled);
}

fn poll<F: Future + Unpin>(future: &mut F, waker: &Waker) -> Poll<F::Output> {
    Pin::new(future).poll(&mut Context::from_waker(waker))
}

fn join<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Poll::Ready(result) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            return result;
        }
        assert!(Instant::now() < deadline, "tracked future stalled");
        thread::yield_now();
    }
}

struct Harness {
    context: RuntimeContextV1<MockBackend>,
    state: Arc<Mutex<MockState>>,
    handle: RuntimeAsyncProgressHandleV1<MockBackend>,
    receiver: Receiver<RuntimeAsyncEngineCommandV1<MockBackend>>,
    stream: RuntimeStreamIdV1,
    kernel: Arc<crate::TypedRuntimeKernelV1<EmptyArgs>>,
}

impl Harness {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut context = RuntimeContextV1::open(MockBackend {
            state: Arc::clone(&state),
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
        let (sender, receiver) = sync_channel(4);
        let handle = RuntimeAsyncProgressHandleV1 {
            observer: RuntimeAsyncEngineHandleV1 {
                context_generation: context.capture_context_generation_v1(),
                capture_budget: None,
                reply_budget: reply_budget::ReplyBudgetV1::new(DEFAULT_RUNTIME_ASYNC_REPLIES_V1),
                admission: drain::AdmissionV1::new(),
                graph_slot: Arc::new(AtomicBool::new(false)),
                snapshot_budget: snapshot::SnapshotBudgetV1::new(
                    DEFAULT_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1,
                ),
                sender,
                worker_thread: Arc::new(OnceLock::new()),
                quarantine_command_panics: true,
            },
        };
        Self {
            context,
            state,
            handle,
            receiver,
            stream,
            kernel,
        }
    }

    fn launch(&self) -> RuntimeAsyncTrackedOperationV1<EmptyArgs, MockError> {
        self.handle
            .launch_tracked(
                self.stream,
                Arc::clone(&self.kernel),
                EmptyArgs,
                geometry(),
                Vec::new(),
            )
            .unwrap()
    }

    fn receive(&self) -> Box<dyn EngineOperationV1<MockBackend>> {
        self.receive_factory().materialize()
    }

    fn receive_factory(&self) -> Box<dyn operation::EngineOperationFactoryV1<MockBackend>> {
        match self.receiver.recv_timeout(Duration::from_secs(1)).unwrap() {
            RuntimeAsyncEngineCommandV1::Operation(factory) => factory,
            _ => panic!("expected typed operation"),
        }
    }

    fn succeed(&self) {
        for status in self.state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
    }
}

#[test]
fn r62_cancel_in_channel_and_registry_never_calls_submit() {
    for registered in [false, true] {
        let mut h = Harness::new();
        let future = h.launch();
        let control = future.control();
        let mut registry = OperationRegistryV1::new(1, false);
        if registered {
            registry.insert(h.receive());
        }
        assert_eq!(
            control.cancel_before_submission(),
            Cancel::CancelledBeforeSubmission
        );
        assert_eq!(control.cancel_before_submission(), Cancel::AlreadyCancelled);
        if !registered {
            registry.insert(h.receive());
        }
        assert_eq!(registry.len(), 1);
        operation::advance_operations_v1(
            &mut h.context,
            &mut registry,
            1,
            1,
            flush_stream_v1::<MockBackend>,
        );
        assert_eq!(registry.len(), 0);
        assert!(h.state.lock().unwrap().flush_calls.is_empty());
        assert!(matches!(
            join(future),
            Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
        ));
        assert_eq!(control.phase(), Phase::CancelledBeforeSubmission);
        assert!(h.state.lock().unwrap().statuses.is_empty());
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn r62_opaque_control_is_exact_and_survives_observer_drop() {
    let mut h = Harness::new();
    let first = h.launch();
    let first_control = first.control();
    assert!(first_control.same_operation(&first.control()));
    let second = h.launch();
    assert!(!first_control.same_operation(&second.control()));
    drop(first);
    assert_eq!(
        first_control.cancel_before_submission(),
        Cancel::CancelledBeforeSubmission
    );
    assert!(h.receive().advance(&mut h.context));
    let mut operation = h.receive();
    assert!(!operation.advance(&mut h.context));
    assert_eq!(h.state.lock().unwrap().statuses.len(), 1);
    h.succeed();
    assert!(operation.advance(&mut h.context));
    assert_eq!(
        join(second).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r62_cancel_vs_owner_race_has_one_winner_and_no_replay() {
    for _ in 0..64 {
        let mut h = Harness::new();
        let future = h.launch();
        let control = future.control();
        let mut factory = h.receive_factory();
        let barrier = Arc::new(Barrier::new(2));
        let owner_barrier = Arc::clone(&barrier);
        let state = Arc::clone(&h.state);
        let owner = thread::spawn(move || {
            let mut operation = factory.materialize();
            owner_barrier.wait();
            let retired = operation.advance(&mut h.context);
            if !retired {
                h.succeed();
                assert!(operation.advance(&mut h.context));
            }
            assert!(h.context.cleanup().is_complete());
        });
        barrier.wait();
        let cancellation = control.cancel_before_submission();
        owner.join().unwrap();
        match cancellation {
            Cancel::CancelledBeforeSubmission => {
                assert_eq!(control.phase(), Phase::CancelledBeforeSubmission);
                assert!(matches!(
                    join(future),
                    Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
                ));
                assert_eq!(state.lock().unwrap().release_calls, 0);
            }
            Cancel::NotCancellable(_) => {
                assert_eq!(control.phase(), Phase::ObservationFinished);
                assert!(join(future).unwrap().submission.is_some());
                assert_eq!(state.lock().unwrap().release_calls, 1);
            }
            other => panic!("unexpected cancellation: {other:?}"),
        }
    }
}

#[test]
fn r62_timeout_before_and_after_submission_recovers_same_operation() {
    for submitted in [false, true] {
        let mut h = Harness::new();
        let future = h.launch();
        let control = future.control();
        let mut operation = h.receive();
        if submitted {
            assert!(!operation.advance(&mut h.context));
        }
        let future = match join(future.observe_with_timeout(ready(()))) {
            Timeout::TimedOut { operation } => operation,
            Timeout::Completed(_) => panic!("pending operation completed"),
        };
        assert!(control.same_operation(&future.control()));
        assert_eq!(
            control.phase(),
            if submitted {
                Phase::Observing
            } else {
                Phase::Queued
            }
        );
        assert_eq!(h.state.lock().unwrap().release_calls, 0);
        if !submitted {
            assert!(!operation.advance(&mut h.context));
        }
        assert!(matches!(
            control.cancel_before_submission(),
            Cancel::NotCancellable(Phase::Observing)
        ));
        h.succeed();
        assert!(operation.advance(&mut h.context));
        let result = join(future).unwrap();
        assert_eq!(
            result.observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert!(result.submission.is_some());
        assert_eq!(h.state.lock().unwrap().statuses.len(), 1);
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn r62_ready_result_wins_and_timer_need_not_be_unpin() {
    struct PinnedTimer {
        _pin: PhantomPinned,
    }
    impl Future for PinnedTimer {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            panic!("ready operation must be polled first");
        }
    }
    let mut h = Harness::new();
    let future = h.launch();
    let mut operation = h.receive();
    assert!(!operation.advance(&mut h.context));
    h.succeed();
    assert!(operation.advance(&mut h.context));
    assert!(matches!(
        join(future.observe_with_timeout(PinnedTimer {
            _pin: PhantomPinned
        })),
        Timeout::Completed(Ok(_))
    ));
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r62_timeout_unregisters_old_waker_without_consuming_concurrent_result() {
    let mut h = Harness::new();
    let future = h.launch();
    let mut operation = h.receive();
    assert!(!operation.advance(&mut h.context));
    let old = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let old_waker = Waker::from(Arc::clone(&old));
    let mut observer = future.observe_with_timeout(ready(()));
    let mut future = match poll(&mut observer, &old_waker) {
        Poll::Ready(Timeout::TimedOut { operation }) => operation,
        _ => panic!("expected timeout"),
    };
    h.succeed();
    assert!(operation.advance(&mut h.context));
    assert_eq!(old.0.load(AtomicOrdering::SeqCst), 0);
    assert!(matches!(
        poll(&mut future, Waker::noop()),
        Poll::Ready(Ok(_))
    ));
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r62_timer_wakeup_and_replacement_use_executor_waker() {
    struct Timer(Arc<Mutex<(bool, Option<Waker>)>>);
    impl Future for Timer {
        type Output = ();
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let mut state = self.0.lock().unwrap();
            if state.0 {
                Poll::Ready(())
            } else {
                state.1 = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
    let h = Harness::new();
    let future = h.launch();
    let timer = Arc::new(Mutex::new((false, None)));
    let mut observer = future.observe_with_timeout(Timer(Arc::clone(&timer)));
    let old = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let new = Arc::new(WakeCounter(AtomicUsize::new(0)));
    for counter in [&old, &new] {
        assert!(poll(&mut observer, &Waker::from(Arc::clone(counter))).is_pending());
    }
    let waker = {
        let mut state = timer.lock().unwrap();
        state.0 = true;
        state.1.take().unwrap()
    };
    waker.wake();
    assert_eq!(old.0.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(new.0.load(AtomicOrdering::SeqCst), 1);
    assert!(matches!(
        poll(&mut observer, Waker::noop()),
        Poll::Ready(Timeout::TimedOut { .. })
    ));
}

#[test]
fn r62_drop_pending_timeout_observer_does_not_withdraw_progress() {
    let mut h = Harness::new();
    let future = h.launch();
    let control = future.control();
    let mut observer = future.observe_with_timeout(pending::<()>());
    let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
    assert!(poll(&mut observer, &Waker::from(Arc::clone(&counter))).is_pending());
    drop(observer);
    let mut operation = h.receive();
    assert!(!operation.advance(&mut h.context));
    h.succeed();
    assert!(operation.advance(&mut h.context));
    assert_eq!(counter.0.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(control.phase(), Phase::ObservationFinished);
    assert_eq!(h.state.lock().unwrap().release_calls, 0);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r62_stop_and_rejection_keep_cancel_and_submission_dispositions_distinct() {
    for submitted in [false, true] {
        for cancel in [false, true] {
            let mut h = Harness::new();
            let future = h.launch();
            let control = future.control();
            let mut operation = h.receive();
            if submitted {
                assert!(!operation.advance(&mut h.context));
            }
            if cancel {
                control.cancel_before_submission();
            }
            drop(operation);
            let result = join(future);
            if cancel && !submitted {
                assert!(matches!(
                    result,
                    Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
                ));
                assert_eq!(control.phase(), Phase::CancelledBeforeSubmission);
            } else {
                assert!(matches!(
                    result,
                    Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
                ));
                assert_eq!(
                    control.phase(),
                    if submitted {
                        Phase::StoppedAfterSubmission
                    } else {
                        Phase::StoppedBeforeSubmission
                    }
                );
            }
            assert!(h.context.cleanup().is_complete());
        }
    }
    let h = Harness::new();
    let future = h.launch();
    let control = future.control();
    h.receive()
        .reject(RuntimeAsyncEngineCallErrorV1::OperationCapacity);
    assert!(matches!(
        join(future),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    ));
    assert_eq!(control.phase(), Phase::StoppedBeforeSubmission);
}

#[test]
fn r62_submission_panic_cannot_reopen_cancellation_or_retry() {
    struct PanickingArgs;
    impl RuntimeArgumentsV1 for PanickingArgs {
        const SIGNATURE_V1: [u8; 32] = [73; 32];
        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            panic!("argument encoder panic");
        }
        fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
            Vec::new()
        }
    }
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(
        Arc::clone(&state),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
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
                        .resolve_kernel::<PanickingArgs>(module, "panic")
                        .unwrap(),
                ),
            )
        })
        .unwrap();
    let future = handle
        .launch_tracked(stream, kernel, PanickingArgs, geometry(), Vec::new())
        .unwrap();
    let control = future.control();
    assert!(matches!(
        join(future),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    ));
    assert_eq!(control.phase(), Phase::StoppedAfterSubmission);
    assert_eq!(
        control.cancel_before_submission(),
        Cancel::NotCancellable(Phase::StoppedAfterSubmission)
    );
    assert!(state.lock().unwrap().statuses.is_empty());
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
}

#[test]
fn r62_timeout_racing_completion_retains_exactly_one_result() {
    for _ in 0..64 {
        let mut h = Harness::new();
        let future = h.launch();
        let control = future.control();
        let mut factory = h.receive_factory();
        let barrier = Arc::new(Barrier::new(2));
        let owner_barrier = Arc::clone(&barrier);
        let owner = thread::spawn(move || {
            let mut operation = factory.materialize();
            assert!(!operation.advance(&mut h.context));
            h.succeed();
            owner_barrier.wait();
            assert!(operation.advance(&mut h.context));
            h
        });
        barrier.wait();
        let result = match join(future.observe_with_timeout(ready(()))) {
            Timeout::Completed(result) => result,
            Timeout::TimedOut { operation } => {
                assert!(control.same_operation(&operation.control()));
                join(operation)
            }
        }
        .unwrap();
        assert_eq!(
            result.observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert!(result.submission.is_some());
        assert_eq!(control.phase(), Phase::ObservationFinished);
        let mut h = owner.join().unwrap();
        assert_eq!(h.state.lock().unwrap().statuses.len(), 1);
        assert_eq!(h.state.lock().unwrap().release_calls, 0);
        assert!(h.context.cleanup().is_complete());
    }
}
