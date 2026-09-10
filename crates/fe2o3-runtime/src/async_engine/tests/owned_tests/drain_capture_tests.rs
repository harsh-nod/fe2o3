use super::*;
use crate::{
    RuntimeAsyncDrainCaptureAdmissionErrorV1 as AdmissionError,
    RuntimeHostCaptureErrorV1 as CaptureError, RuntimeHostCaptureSourceV1,
};

fn capture_config(bytes: usize) -> RuntimeAsyncEngineConfigV1 {
    RuntimeAsyncEngineConfigV1::default()
        .with_drain_capture_byte_capacity(bytes)
        .unwrap()
}

fn capture_source(
    handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
) -> RuntimeHostCaptureSourceV1 {
    handle
        .observer()
        .try_with_context(|context| {
            let allocation = context
                .allocate(
                    context.devices()[0].id(),
                    RuntimeMemoryKindV1::HostVisible,
                    64,
                    8,
                )
                .unwrap();
            context
                .prepare_host_drain_capture_v1(allocation, 8, 16)
                .unwrap()
        })
        .unwrap()
}

#[test]
fn drn1a_transferable_owner_preserves_registration_and_default_unsupported_spi() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let mut context = RuntimeContextV1::open(MockBackend {
        state: state.clone(),
    })
    .unwrap();
    let allocation = context
        .allocate(
            context.devices()[0].id(),
            RuntimeMemoryKindV1::HostVisible,
            64,
            8,
        )
        .unwrap();
    let source = context
        .prepare_host_drain_capture_v1(allocation, 8, 16)
        .unwrap();
    let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        capture_config(64),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let report = join_command(
        handle
            .begin_drain_with_capture(64, source, vec![0; 16].into_boxed_slice())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.drain.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert!(matches!(
        report.capture,
        Err(CaptureError::UnsupportedBackend)
    ));
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    assert!(state.lock().unwrap().issues.is_empty());
    let mut context = engine.into_context().unwrap();
    assert!(!context.is_terminal());
    assert!(context.cleanup().is_complete());
}

#[test]
fn drn1a_capture_resolves_late_backing_and_bytes_outlive_reply_and_owner() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace {
        capture_backing_pending: true,
        ..OwnerTrace::default()
    }));
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), capture_config(64));
    let source = capture_source(&handle);
    assert_eq!(trace.lock().unwrap().capture_calls, 0);
    let release = paused_owner(&handle);
    let late = trace.clone();
    let accepted = handle
        .observer()
        .enqueue_with_context(move |_| {
            // Script the native backing becoming current only after registration.
            late.lock().unwrap().capture_backing_pending = false;
        })
        .unwrap();
    let future = handle
        .begin_drain_with_capture(64, source, vec![0xcc; 16].into_boxed_slice())
        .unwrap();
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    release.wait();
    let report = join_command(future).unwrap();
    assert_eq!(report.drain.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    let captured = report.capture.unwrap();
    assert_eq!(captured.as_bytes(), &[0x5a; 16]);
    join_command(accepted).unwrap();
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
    let trace = trace.lock().unwrap();
    assert_eq!(trace.capture_calls, 1);
    assert_eq!(trace.capture_requests.len(), 1);
    assert_eq!(
        (
            trace.capture_requests[0].0,
            trace.capture_requests[0].2,
            trace.capture_requests[0].3
        ),
        (1, 8, 16)
    );
    assert!(
        !trace
            .calls
            .iter()
            .any(|(call, _)| *call == "read_allocation_v1")
    );
    let captured_at = trace
        .calls
        .iter()
        .position(|(call, _)| *call == "capture")
        .unwrap();
    let released_at = trace
        .calls
        .iter()
        .position(|(call, _)| *call == "release_allocation_v1")
        .unwrap();
    assert!(captured_at < released_at);
    assert!(state.lock().unwrap().issues.is_empty());
    drop(trace);
    drop(captured);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
}

#[test]
fn drn1a_admission_rejection_returns_original_destination_without_closing() {
    for mode in 0..6 {
        let capacity = match mode {
            0 => 0,
            3 => 8,
            _ => 64,
        };
        let config = capture_config(capacity).with_reply_capacity(1).unwrap();
        let (engine, handle) = start_with_config(
            Arc::new(Mutex::new(MockState::default())),
            Arc::new(Mutex::new(OwnerTrace::default())),
            config,
        );
        let source = capture_source(&handle);
        let mut foreign = None;
        let source = if mode == 5 {
            let mut context = RuntimeContextV1::open(MockBackend {
                state: Arc::new(Mutex::new(MockState::default())),
            })
            .unwrap();
            let allocation = context
                .allocate(
                    context.devices()[0].id(),
                    RuntimeMemoryKindV1::HostVisible,
                    64,
                    8,
                )
                .unwrap();
            let source = context
                .prepare_host_drain_capture_v1(allocation, 8, 16)
                .unwrap();
            foreign = Some(context);
            source
        } else {
            source
        };
        let allocation = source.allocation();
        let held_reply =
            (mode == 4).then(|| handle.observer().enqueue_with_context(|_| ()).unwrap());
        let destination = vec![0xc7; if mode == 2 { 8 } else { 16 }].into_boxed_slice();
        let original = destination.as_ptr();
        let failure = handle
            .begin_drain_with_capture(if mode == 1 { 0 } else { 64 }, source, destination)
            .err()
            .unwrap();
        let expected = match mode {
            0 => AdmissionError::Disabled,
            1 => AdmissionError::InvalidTickBudget,
            2 => AdmissionError::InvalidDestination,
            3 => AdmissionError::StorageCapacity,
            4 => AdmissionError::ReplyCapacity,
            5 => AdmissionError::ForeignContext,
            _ => unreachable!(),
        };
        assert_eq!(failure.error(), expected);
        let (_, source, destination) = failure.into_parts();
        assert_eq!(source.allocation(), allocation);
        assert_eq!(destination.as_ptr(), original);
        assert!(destination.iter().all(|byte| *byte == 0xc7));
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        if let Some(reply) = held_reply {
            join_command(reply).unwrap();
            // Delivery can precede the producer's final reply-cell disposal.
            handle.observer().try_with_context(|_| ()).unwrap();
            assert_eq!(handle.observer().reply_cells_in_use(), 0);
        }
        assert_eq!(
            join_command(handle.observer().enqueue_with_context(|_| 7).unwrap()),
            Ok(7)
        );
        assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
        if let Some(mut context) = foreign {
            assert!(context.cleanup().is_complete());
        }
    }
}

#[test]
fn drn1a_released_source_cannot_read_replacement_allocation() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        trace.clone(),
        capture_config(64),
    );
    let source = capture_source(&handle);
    let allocation = source.allocation();
    let replacement = handle
        .observer()
        .try_with_context(move |context| {
            context.release_allocation(allocation).unwrap();
            context
                .allocate(
                    context.devices()[0].id(),
                    RuntimeMemoryKindV1::HostVisible,
                    64,
                    8,
                )
                .unwrap()
        })
        .unwrap();
    assert_ne!(allocation, replacement);
    let report = join_command(
        handle
            .begin_drain_with_capture(64, source, vec![0; 16].into_boxed_slice())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.drain.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert!(matches!(
        report.capture,
        Err(CaptureError::UnknownAllocation)
    ));
    assert_eq!(trace.lock().unwrap().capture_calls, 0);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
}

#[test]
fn drn1a_capture_rejection_terminal_failure_and_panic_never_deliver_partial_bytes() {
    for mode in 0..4 {
        let trace = Arc::new(Mutex::new(OwnerTrace {
            capture_failure: match mode {
                0 => Some(CaptureError::NativeRejected),
                1 => Some(CaptureError::NativeUncertain),
                2 => Some(CaptureError::Pending),
                _ => None,
            },
            capture_terminal: mode == 1,
            capture_panics: mode == 3,
            ..OwnerTrace::default()
        }));
        let (engine, handle) = start_with_config(
            Arc::new(Mutex::new(MockState::default())),
            trace.clone(),
            capture_config(64),
        );
        let source = capture_source(&handle);
        let result = join_command(
            handle
                .begin_drain_with_capture(64, source, vec![0; 16].into_boxed_slice())
                .unwrap(),
        );
        match mode {
            0 | 2 => {
                let report = result.unwrap();
                assert_eq!(report.drain.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
                assert!(
                    matches!(report.capture, Err(error) if error == if mode == 0 { CaptureError::NativeRejected } else { CaptureError::Pending })
                );
            }
            1 => assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::CaptureFailed(
                    CaptureError::NativeUncertain
                ))
            )),
            3 => assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            )),
            _ => unreachable!(),
        }
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        let shutdown = engine.shutdown().unwrap();
        assert_eq!(
            shutdown.disposition,
            if mode == 1 || mode == 3 {
                RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
            } else {
                RuntimeAsyncOwnedDispositionV1::Released
            }
        );
        let trace = trace.lock().unwrap();
        assert_eq!(trace.capture_calls, 1);
        if mode == 1 || mode == 3 {
            assert!(
                !trace
                    .calls
                    .iter()
                    .any(|(call, _)| *call == "release_allocation_v1")
            );
        }
    }
}

#[test]
fn drn1a_budget_exhaustion_discards_destination_without_capture_or_native_release() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), capture_config(64));
    let source = capture_source(&handle);
    let (stream, kernel) = launch_fixture(&handle);
    handle
        .observer()
        .try_with_context(move |context| {
            context
                .launch(stream, &kernel, &EmptyArgs, geometry(), &[])
                .unwrap();
        })
        .unwrap();
    let report = join_command(
        handle
            .begin_drain_with_capture(1, source, vec![0; 16].into_boxed_slice())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        report.drain.outcome,
        RuntimeAsyncDrainOutcomeV1::BudgetExhausted
    );
    assert!(matches!(
        report.capture,
        Err(CaptureError::CaptureIncomplete)
    ));
    assert_eq!(report.drain.retained_submissions.pending, 1);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    assert_eq!(state.lock().unwrap().issues.len(), 1);
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    let trace = trace.lock().unwrap();
    assert_eq!(trace.capture_calls, 0);
    assert!(
        !trace
            .calls
            .iter()
            .any(|(call, _)| *call == "release_allocation_v1")
    );
}

#[test]
fn drn1a_capture_and_plain_drain_race_share_exactly_one_cutoff() {
    for _ in 0..8 {
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (engine, handle) = start_with_config(
            Arc::new(Mutex::new(MockState::default())),
            trace.clone(),
            capture_config(64),
        );
        let source = capture_source(&handle);
        let release = paused_owner(&handle);
        let barrier = Arc::new(Barrier::new(3));
        let first_handle = handle.clone();
        let first_barrier = barrier.clone();
        let capture = thread::spawn(move || {
            first_barrier.wait();
            first_handle.begin_drain_with_capture(64, source, vec![0xa7; 16].into_boxed_slice())
        });
        let second_handle = handle.clone();
        let second_barrier = barrier.clone();
        let plain = thread::spawn(move || {
            second_barrier.wait();
            second_handle.begin_drain(64)
        });
        barrier.wait();
        let capture = capture.join().unwrap();
        let plain = plain.join().unwrap();
        assert_ne!(capture.is_ok(), plain.is_ok());
        assert!(matches!(
            handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        release.wait();
        match capture {
            Ok(future) => {
                assert!(matches!(
                    plain,
                    Err(RuntimeAsyncDrainErrorV1::AdmissionClosed)
                ));
                drop(join_command(future).unwrap().capture.unwrap());
                assert_eq!(trace.lock().unwrap().capture_calls, 1);
            }
            Err(failure) => {
                assert_eq!(failure.error(), AdmissionError::AdmissionClosed);
                let (_, _, destination) = failure.into_parts();
                assert_eq!(&*destination, &[0xa7; 16]);
                assert_eq!(
                    join_command(plain.unwrap()).unwrap().outcome,
                    RuntimeAsyncDrainOutcomeV1::Quiescent
                );
                assert_eq!(trace.lock().unwrap().capture_calls, 0);
            }
        }
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    }
}

#[test]
fn drn1a_abandoned_capture_observer_does_not_withdraw_capture() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        trace.clone(),
        capture_config(64),
    );
    let source = capture_source(&handle);
    let release = paused_owner(&handle);
    drop(
        handle
            .begin_drain_with_capture(64, source, vec![0; 16].into_boxed_slice())
            .unwrap(),
    );
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
    release.wait();
    let deadline = Instant::now() + Duration::from_secs(5);
    while handle.observer().drain_capture_bytes_in_use() != 0 {
        assert!(
            Instant::now() < deadline,
            "abandoned capture did not dispose its result"
        );
        thread::yield_now();
    }
    assert_eq!(trace.lock().unwrap().capture_calls, 1);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
}

#[derive(Clone, Copy, Debug)]
enum PrefixCaptureOutcome {
    Success,
    Rejected,
    Terminal,
}

impl PrefixCaptureOutcome {
    fn trace(self) -> OwnerTrace {
        OwnerTrace {
            capture_failure: match self {
                Self::Success => None,
                Self::Rejected => Some(CaptureError::NativeRejected),
                Self::Terminal => Some(CaptureError::NativeUncertain),
            },
            capture_terminal: matches!(self, Self::Terminal),
            ..OwnerTrace::default()
        }
    }

    fn check(
        self,
        result: Result<crate::RuntimeAsyncDrainCaptureReportV1, RuntimeAsyncEngineCallErrorV1>,
        retained_submissions: usize,
    ) -> Option<crate::RuntimeAsyncCapturedBytesV1> {
        if matches!(self, Self::Terminal) {
            assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::CaptureFailed(
                    CaptureError::NativeUncertain
                ))
            ));
            return None;
        }
        let report = result.unwrap();
        assert_eq!(report.drain.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
        assert!(report.drain.queued_commands_exhausted);
        assert_eq!(report.drain.operations_remaining, 0);
        assert!(!report.drain.graph_active);
        assert_eq!(
            report.drain.retained_submissions.total_submissions,
            retained_submissions
        );
        assert_eq!(
            report.drain.retained_submissions.succeeded,
            retained_submissions
        );
        assert_eq!(report.drain.retained_submissions.pending, 0);
        match self {
            Self::Success => {
                let captured = report.capture.unwrap();
                assert_eq!(captured.as_bytes(), &[0x5a; 16]);
                Some(captured)
            }
            Self::Rejected => {
                assert!(matches!(report.capture, Err(CaptureError::NativeRejected)));
                None
            }
            Self::Terminal => unreachable!(),
        }
    }
}

struct CapturePrefix {
    streams: [RuntimeStreamIdV1; 2],
    native_streams: [u64; 2],
    native_source: u64,
    kernel: Arc<crate::TypedRuntimeKernelV1<DrainArgs>>,
    arguments: DrainArgs,
    source: Option<RuntimeHostCaptureSourceV1>,
}

impl CapturePrefix {
    fn new(
        handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
        state: &Arc<Mutex<MockState>>,
    ) -> Self {
        let mut fixture = handle
            .observer()
            .try_with_context(|context| {
                let device = context.devices()[0].id();
                context
                    .configure_allocation_admission_v1(device, 128, 2)
                    .unwrap();
                let streams = [
                    context.create_stream(device).unwrap(),
                    context.create_stream(device).unwrap(),
                ];
                let module = context.load_module(device, &[1]).unwrap();
                let kernel = Arc::new(
                    context
                        .resolve_kernel::<DrainArgs>(module, "capture-prefix")
                        .unwrap(),
                );
                let allocation = context
                    .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
                    .unwrap();
                context.write_allocation(allocation, 0, &[0; 64]).unwrap();
                let host = context
                    .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
                    .unwrap();
                let source = context.prepare_host_drain_capture_v1(host, 8, 16).unwrap();
                let usage = context
                    .allocation_admission_usage_v1(device)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    usage.used,
                    crate::RuntimeResourceVectorV1::ZERO
                        .with(crate::RuntimeResourceKindV1::RequestedAllocationBytes, 128)
                        .with(crate::RuntimeResourceKindV1::AllocationRecords, 2)
                );
                assert_eq!(usage.retained_records, 2);
                Self {
                    streams,
                    native_streams: [0; 2],
                    native_source: 0,
                    kernel,
                    arguments: DrainArgs(allocation),
                    source: Some(source),
                }
            })
            .unwrap();
        let state = state.lock().unwrap();
        assert!(state.issues.is_empty());
        assert_eq!(state.created_streams.len(), 2);
        fixture
            .native_streams
            .copy_from_slice(&state.created_streams);
        // Host allocation is the final native-ID-producing setup operation.
        fixture.native_source = state.next;
        fixture
    }

    fn request(
        &self,
        stream: usize,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> RuntimeAsyncLaunchRequestV1<DrainArgs> {
        RuntimeAsyncLaunchRequestV1::new(
            self.streams[stream],
            self.kernel.clone(),
            &self.arguments,
            geometry(),
            dependencies,
        )
        .unwrap()
    }

    fn raw_predecessor(
        &self,
        handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
    ) -> (crate::RuntimeSubmissionIdV1, RuntimeEventIdV1) {
        let (stream, kernel, allocation) = (self.streams[0], self.kernel.clone(), self.arguments.0);
        handle
            .observer()
            .try_with_context(move |context| {
                let raw = context
                    .launch(stream, &kernel, &DrainArgs(allocation), geometry(), &[])
                    .unwrap();
                (raw.id(), context.record_event(&raw).unwrap())
            })
            .unwrap()
    }

    fn graph_request(
        &self,
        handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
    ) -> (
        crate::RuntimeGraphRequestV1<ThreadBoundBackend>,
        crate::completion::ContextIdentityV1,
        crate::completion::CompletionGraphIdentityV1,
    ) {
        use crate::completion::{
            CompletionGraphV1, CompletionNodeV1, EventIdentityV1, FutureIdentityV1,
        };
        let (streams, kernel, allocation) = (self.streams, self.kernel.clone(), self.arguments.0);
        handle
            .observer()
            .try_with_context(move |context| {
                let identities = streams
                    .iter()
                    .map(|&stream| context.completion_stream_identity_v1(stream).unwrap())
                    .collect::<Vec<_>>();
                let context_id = identities[0].context();
                let event = EventIdentityV1::new(context_id, [0x69; 32]);
                let graph = CompletionGraphV1::new(
                    context_id,
                    identities.clone(),
                    vec![
                        CompletionNodeV1::future(
                            capture_node(1),
                            FutureIdentityV1::new(identities[0], [1; 32]),
                            None,
                        ),
                        CompletionNodeV1::record_event(
                            capture_node(2),
                            identities[0],
                            event,
                            Some(capture_node(1)),
                        ),
                        CompletionNodeV1::wait_event(
                            capture_node(3),
                            identities[1],
                            event,
                            capture_node(2),
                            None,
                        ),
                        CompletionNodeV1::future(
                            capture_node(4),
                            FutureIdentityV1::new(identities[1], [4; 32]),
                            Some(capture_node(3)),
                        ),
                    ],
                )
                .unwrap();
                let graph_id = graph.identity();
                let mut request = crate::RuntimeGraphRequestV1::new(
                    graph,
                    identities.into_iter().zip(streams).collect(),
                )
                .unwrap();
                for node in [1, 4] {
                    request
                        .bind_launch(
                            capture_node(node),
                            kernel.clone(),
                            &DrainArgs(allocation),
                            geometry(),
                        )
                        .unwrap();
                }
                (request, context_id, graph_id)
            })
            .unwrap()
    }
}

fn capture_node(value: u32) -> crate::completion::CompletionNodeIdV1 {
    crate::completion::CompletionNodeIdV1::new(value).unwrap()
}

#[derive(Default)]
struct CaptureWakeCount(AtomicUsize);

impl Wake for CaptureWakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, AtomicOrdering::SeqCst);
    }
}

fn arm_capture_future<F: Future>(mut future: Pin<&mut F>) -> Arc<CaptureWakeCount> {
    let count = Arc::new(CaptureWakeCount::default());
    let waker = Waker::from(count.clone());
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    count
}

fn await_capture_future<F: Future>(
    mut future: Pin<&mut F>,
    count: &Arc<CaptureWakeCount>,
) -> F::Output {
    let waker = Waker::from(count.clone());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return result;
        }
        assert!(Instant::now() < deadline, "composed capture future stalled");
        thread::yield_now();
    }
}

fn assert_capture_shutdown(
    engine: RuntimeAsyncOwnedEngineV1<ThreadBoundBackend>,
    outcome: PrefixCaptureOutcome,
    retained_submissions: usize,
) {
    let shutdown = engine.shutdown().unwrap();
    let cleanup = shutdown.cleanup.unwrap();
    if matches!(outcome, PrefixCaptureOutcome::Terminal) {
        assert_eq!(
            shutdown.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert!(cleanup.is_terminal());
        assert_eq!(cleanup.retained().allocations, 2);
        assert_eq!(cleanup.retained().submissions, retained_submissions);
        assert_eq!(cleanup.retained().modules, 1);
        assert_eq!(cleanup.retained().streams, 2);
        assert_eq!(cleanup.allocation_credit_records_v1(), 2);
    } else {
        assert_eq!(
            shutdown.disposition,
            RuntimeAsyncOwnedDispositionV1::Released
        );
        assert!(cleanup.is_complete());
        assert_eq!(cleanup.allocation_credit_records_v1(), 0);
    }
}

#[test]
fn drn3b_multistream_operation_waiter_prefix_precedes_capture_outcomes() {
    for reject_submit in [false, true] {
        for outcome in [
            PrefixCaptureOutcome::Success,
            PrefixCaptureOutcome::Rejected,
            PrefixCaptureOutcome::Terminal,
        ] {
            let state = Arc::new(Mutex::new(MockState::default()));
            let trace = Arc::new(Mutex::new(outcome.trace()));
            let (engine, handle) =
                start_with_config(state.clone(), trace.clone(), capture_config(16));
            let mut prefix = CapturePrefix::new(&handle, &state);
            let (predecessor, event) = prefix.raw_predecessor(&handle);
            let mut waiter = Box::pin(handle.observer().event_future(event).unwrap());
            assert_eq!(waiter.event(), event);
            let waiter_wakes = arm_capture_future(waiter.as_mut());
            let release = paused_owner(&handle);
            let request = prefix.request(1, vec![event]);
            let bytes = request.snapshot_bytes();
            let mut cancelled = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
            let cancelled_control = cancelled.control();
            let cancelled_wakes = arm_capture_future(cancelled.as_mut());
            let mut sibling = Box::pin(
                handle
                    .enqueue_launch_tracked(prefix.request(1, vec![event]))
                    .unwrap(),
            );
            let sibling_control = sibling.control();
            let sibling_wakes = arm_capture_future(sibling.as_mut());
            assert!(!cancelled_control.same_operation(&sibling_control));
            assert_eq!(
                cancelled_control.cancel_before_submission(),
                Cancel::CancelledBeforeSubmission
            );
            assert_eq!(
                cancelled_control.cancel_before_submission(),
                Cancel::AlreadyCancelled
            );
            assert_eq!(sibling_control.phase(), Phase::Queued);
            if reject_submit {
                state
                    .lock()
                    .unwrap()
                    .submit_failures
                    .push_back(RuntimeBackendFailureV1::Rejected(MockError(
                        "capture-prefix-submit",
                    )));
            }
            let destination = vec![0xc7; 16].into_boxed_slice();
            let destination_pointer = destination.as_ptr();
            let mut capture = Box::pin(
                handle
                    .begin_drain_with_capture(128, prefix.source.take().unwrap(), destination)
                    .unwrap(),
            );
            let capture_wakes = arm_capture_future(capture.as_mut());
            assert_eq!(handle.observer().snapshot_bytes_in_use(), 2 * bytes);
            assert_eq!(handle.observer().reply_cells_in_use(), 4);
            assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
            assert!(matches!(
                handle.enqueue_launch_tracked(prefix.request(1, vec![event])),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
            assert_eq!(handle.observer().snapshot_bytes_in_use(), 2 * bytes);
            assert_eq!(handle.observer().reply_cells_in_use(), 4);
            state.lock().unwrap().complete_on_flush = true;
            release.wait();
            let native_count = if reject_submit { 1 } else { 2 };
            let captured = outcome.check(
                await_capture_future(capture.as_mut(), &capture_wakes),
                native_count,
            );
            assert!(matches!(
                await_capture_future(cancelled.as_mut(), &cancelled_wakes),
                Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
            ));
            let sibling_result = await_capture_future(sibling.as_mut(), &sibling_wakes).unwrap();
            if reject_submit {
                assert!(sibling_result.submission.is_none());
                assert!(matches!(
                    sibling_result.observation,
                    Err(RuntimeErrorV1::BackendRejected(MockError(
                        "capture-prefix-submit"
                    )))
                ));
            } else {
                let submission = sibling_result.submission.unwrap();
                assert_ne!(submission.id(), predecessor);
                assert_eq!(submission.stream(), prefix.streams[1]);
                assert_eq!(
                    sibling_result.observation.unwrap(),
                    RuntimeCompletionStatusV1::Succeeded
                );
            }
            assert_eq!(sibling_result.rejected_observations, 0);
            assert_eq!(sibling_result.last_rejected_observation, None);
            assert_eq!(
                await_capture_future(waiter.as_mut(), &waiter_wakes).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(waiter.event(), event);
            assert!(cancelled_control.same_operation(&cancelled.control()));
            assert!(sibling_control.same_operation(&sibling.control()));
            assert_eq!(cancelled_control.phase(), Phase::CancelledBeforeSubmission);
            assert_eq!(sibling_control.phase(), Phase::ObservationFinished);
            assert_capture_shutdown(engine, outcome, native_count);
            assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
            assert_eq!(handle.observer().reply_cells_in_use(), 3);
            for wakes in [
                &capture_wakes,
                &cancelled_wakes,
                &sibling_wakes,
                &waiter_wakes,
            ] {
                assert_eq!(wakes.0.load(AtomicOrdering::SeqCst), 1);
            }
            drop((capture, cancelled, sibling, waiter));
            assert_eq!(handle.observer().reply_cells_in_use(), 0);
            assert_eq!(
                handle.observer().drain_capture_bytes_in_use(),
                if captured.is_some() { 16 } else { 0 }
            );
            if let Some(captured) = captured {
                assert_eq!(captured.as_bytes().as_ptr(), destination_pointer);
                assert_eq!(captured.as_bytes(), &[0x5a; 16]);
                drop(captured);
            }
            assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
            let state = state.lock().unwrap();
            assert_eq!(state.issues.len(), native_count);
            assert_eq!(state.issues[0].0, prefix.native_streams[0]);
            assert!(state.submit_failures.is_empty());
            if !reject_submit {
                assert_eq!(state.issues[1].0, prefix.native_streams[1]);
                assert_ne!(state.issues[0].1, state.issues[1].1);
                let dependencies = &state.submission_dependencies[&state.issues[1].1];
                assert_eq!(dependencies.len(), 1);
                assert_eq!(state.event_sources[&dependencies[0]], state.issues[0].1);
            }
            let mut native_ids: Vec<_> = state.issues.iter().map(|issue| issue.1).collect();
            native_ids.sort_unstable();
            let trace = trace.lock().unwrap();
            assert_eq!(trace.capture_calls, 1);
            assert_eq!(
                trace.capture_requests,
                vec![(1, prefix.native_source, 8, 16)]
            );
            assert_eq!(
                trace.capture_submission_rosters,
                vec![
                    native_ids
                        .iter()
                        .map(|&id| (id, BackendPollV1::Succeeded))
                        .collect::<Vec<_>>()
                ]
            );
            assert!(
                trace
                    .polled_submissions
                    .iter()
                    .all(|id| native_ids.contains(id))
            );
            assert_eq!(
                trace
                    .calls
                    .iter()
                    .filter(|(call, _)| *call == "submit_v1")
                    .count(),
                2
            );
            if matches!(outcome, PrefixCaptureOutcome::Terminal) {
                assert!(trace.released_submissions.is_empty());
                assert_eq!(state.statuses.len(), native_count);
                assert!(!trace.calls.iter().any(|(call, _)| matches!(
                    *call,
                    "release_allocation_v1" | "finalize" | "drop"
                )));
            } else {
                let mut released = trace.released_submissions.clone();
                released.sort_unstable();
                assert_eq!(released, native_ids);
                assert_eq!(state.release_calls, native_count);
                assert!(state.statuses.is_empty());
            }
        }
    }
}

#[test]
fn drn3b_active_graph_releases_exact_reservation_before_capture_outcomes() {
    for outcome in [
        PrefixCaptureOutcome::Success,
        PrefixCaptureOutcome::Rejected,
        PrefixCaptureOutcome::Terminal,
    ] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(outcome.trace()));
        let (engine, handle) = start_with_config(state.clone(), trace.clone(), capture_config(16));
        let mut prefix = CapturePrefix::new(&handle, &state);
        let (request, context_id, graph_id) = prefix.graph_request(&handle);
        let mut graph = Box::pin(handle.submit_graph(request).unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.lock().unwrap().issues.is_empty() {
            assert!(Instant::now() < deadline, "graph prefix was not issued");
            thread::yield_now();
        }
        let release = paused_owner(&handle);
        assert_eq!(state.lock().unwrap().issues.len(), 1);
        assert!(handle.observer.graph_slot.load(Ordering::Acquire));
        let graph_wakes = arm_capture_future(graph.as_mut());
        let mut capture = Box::pin(
            handle
                .begin_drain_with_capture(
                    128,
                    prefix.source.take().unwrap(),
                    vec![0xcc; 16].into_boxed_slice(),
                )
                .unwrap(),
        );
        let capture_wakes = arm_capture_future(capture.as_mut());
        assert_eq!(handle.observer().reply_cells_in_use(), 3);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
        assert_eq!(trace.lock().unwrap().capture_calls, 0);
        assert!(matches!(
            handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        state.lock().unwrap().complete_on_flush = true;
        release.wait();
        let captured = outcome.check(await_capture_future(capture.as_mut(), &capture_wakes), 0);
        let graph_report = await_capture_future(graph.as_mut(), &graph_wakes)
            .unwrap()
            .unwrap();
        assert_eq!(graph_report.execution.context(), context_id);
        assert_eq!(graph_report.execution.graph_identity(), graph_id);
        assert_ne!(graph_report.execution.generation(), 0);
        assert_eq!(
            graph_report.observations,
            vec![
                (capture_node(1), RuntimeCompletionStatusV1::Succeeded),
                (capture_node(4), RuntimeCompletionStatusV1::Succeeded),
            ]
        );
        assert!(graph_report.errors.is_empty());
        assert_eq!(graph_report.rejected_observations, 0);
        assert_eq!(graph_report.rejected_releases, 0);
        assert!(!handle.observer.graph_slot.load(Ordering::Acquire));
        assert_capture_shutdown(engine, outcome, 0);
        assert_eq!(graph_wakes.0.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(capture_wakes.0.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(handle.observer().reply_cells_in_use(), 2);
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        drop((capture, graph));
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        assert_eq!(
            handle.observer().drain_capture_bytes_in_use(),
            if captured.is_some() { 16 } else { 0 }
        );
        drop(captured);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        let state = state.lock().unwrap();
        assert_eq!(state.issues.len(), 2);
        assert_eq!(state.issues[0].0, prefix.native_streams[0]);
        assert_eq!(state.issues[1].0, prefix.native_streams[1]);
        assert_ne!(state.issues[0].1, state.issues[1].1);
        assert!(state.statuses.is_empty());
        assert_eq!(state.release_calls, 2);
        let trace = trace.lock().unwrap();
        assert_eq!(trace.capture_calls, 1);
        assert_eq!(
            trace.capture_requests,
            vec![(1, prefix.native_source, 8, 16)]
        );
        assert_eq!(trace.capture_submission_rosters, vec![vec![]]);
        assert_eq!(
            trace.released_submissions,
            vec![state.issues[0].1, state.issues[1].1]
        );
        assert_eq!(
            trace
                .calls
                .iter()
                .filter(|(call, _)| *call == "submit_v1")
                .count(),
            2
        );
        let captured_at = trace
            .calls
            .iter()
            .position(|(call, _)| *call == "capture")
            .unwrap();
        let retired_at = trace
            .calls
            .iter()
            .rposition(|(call, _)| *call == "release_submission_v1")
            .unwrap();
        assert!(retired_at < captured_at);
        if matches!(outcome, PrefixCaptureOutcome::Terminal) {
            assert!(
                !trace.calls.iter().any(|(call, _)| matches!(
                    *call,
                    "release_allocation_v1" | "finalize" | "drop"
                ))
            );
        } else {
            let freed_at = trace
                .calls
                .iter()
                .position(|(call, _)| *call == "release_allocation_v1")
                .unwrap();
            assert!(captured_at < freed_at);
        }
    }
}

#[test]
fn drn3b_observation_rejection_retains_active_prefix_and_suppresses_capture() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(8, 8, 8, 1, Duration::from_millis(1))
        .unwrap()
        .with_drain_capture_byte_capacity(16)
        .unwrap();
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), config);
    let mut prefix = CapturePrefix::new(&handle, &state);
    let (_, event) = prefix.raw_predecessor(&handle);
    let mut waiter = Box::pin(handle.observer().event_future(event).unwrap());
    let waiter_wakes = arm_capture_future(waiter.as_mut());
    let release = paused_owner(&handle);
    let initial_polls = state.lock().unwrap().poll_calls;
    state.lock().unwrap().poll_failures.extend(
        (0..64).map(|_| RuntimeBackendFailureV1::Rejected(MockError("capture-prefix-observation"))),
    );
    let request = prefix.request(1, vec![event]);
    let bytes = request.snapshot_bytes();
    let mut operation = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
    let control = operation.control();
    let operation_wakes = arm_capture_future(operation.as_mut());
    let mut capture = Box::pin(
        handle
            .begin_drain_with_capture(
                8,
                prefix.source.take().unwrap(),
                vec![0xcc; 16].into_boxed_slice(),
            )
            .unwrap(),
    );
    let capture_wakes = arm_capture_future(capture.as_mut());
    assert_eq!(handle.observer().snapshot_bytes_in_use(), bytes);
    assert_eq!(handle.observer().reply_cells_in_use(), 3);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
    release.wait();
    let report = await_capture_future(capture.as_mut(), &capture_wakes).unwrap();
    assert_eq!(
        report.drain.outcome,
        RuntimeAsyncDrainOutcomeV1::BudgetExhausted
    );
    assert_eq!(report.drain.ticks, 8);
    assert_eq!(report.drain.retained_submissions.total_submissions, 2);
    assert_eq!(report.drain.retained_submissions.pending, 2);
    assert_eq!(report.drain.operations_remaining, 1);
    assert!(report.drain.queued_commands_exhausted);
    assert!(!report.drain.graph_active);
    assert!(matches!(
        report.capture,
        Err(CaptureError::CaptureIncomplete)
    ));
    assert!(matches!(
        await_capture_future(waiter.as_mut(), &waiter_wakes),
        Err(RuntimeAsyncEventErrorV1::Runtime(
            RuntimeErrorV1::BackendRejected(MockError("capture-prefix-observation"))
        ))
    ));
    assert_eq!(waiter.event(), event);
    assert!(matches!(
        await_capture_future(operation.as_mut(), &operation_wakes),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert!(control.same_operation(&operation.control()));
    assert_eq!(control.phase(), Phase::StoppedAfterSubmission);
    assert_eq!(
        control.cancel_before_submission(),
        Cancel::NotCancellable(Phase::StoppedAfterSubmission)
    );
    assert_capture_shutdown(engine, PrefixCaptureOutcome::Terminal, 2);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    for wakes in [&capture_wakes, &operation_wakes, &waiter_wakes] {
        assert_eq!(wakes.0.load(AtomicOrdering::SeqCst), 1);
    }
    drop((capture, operation, waiter));
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let state = state.lock().unwrap();
    assert_eq!(state.issues.len(), 2);
    assert_eq!(state.issues[0].0, prefix.native_streams[0]);
    assert_eq!(state.issues[1].0, prefix.native_streams[1]);
    assert_ne!(state.issues[0].1, state.issues[1].1);
    assert_eq!(state.statuses.len(), 2);
    assert!(
        state
            .statuses
            .values()
            .all(|status| *status == BackendPollV1::Pending)
    );
    assert_eq!(state.release_calls, 0);
    let rejected_polls = state.poll_calls - initial_polls;
    assert!(rejected_polls >= 4);
    assert_eq!(state.poll_failures.len(), 64 - rejected_polls);
    assert!(!state.poll_failures.is_empty());
    let trace = trace.lock().unwrap();
    assert_eq!(trace.capture_calls, 0);
    assert!(trace.capture_requests.is_empty());
    assert!(trace.capture_submission_rosters.is_empty());
    assert!(trace.released_submissions.is_empty());
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "submit_v1")
            .count(),
        2
    );
    for issue in &state.issues {
        assert!(
            trace
                .polled_submissions
                .iter()
                .filter(|&&id| id == issue.1)
                .count()
                >= 2
        );
    }
    assert!(
        trace
            .polled_submissions
            .iter()
            .all(|id| state.statuses.contains_key(id))
    );
    assert!(
        !trace
            .calls
            .iter()
            .any(|(call, _)| matches!(*call, "release_allocation_v1" | "finalize" | "drop"))
    );
}

#[test]
fn drn3b_stop_before_pickup_or_during_capture_obeys_owner_order() {
    for during_capture in [false, true] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (entered_sender, entered_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        if during_capture {
            trace.lock().unwrap().capture_pause = Some((entered_sender, release_receiver));
        }
        let (engine, handle) = start_with_config(state.clone(), trace.clone(), capture_config(16));
        let mut prefix = CapturePrefix::new(&handle, &state);
        let (predecessor, event) = prefix.raw_predecessor(&handle);
        let mut waiter = Box::pin(handle.observer().event_future(event).unwrap());
        let waiter_wakes = arm_capture_future(waiter.as_mut());
        let release_owner = paused_owner(&handle);
        let request = prefix.request(1, vec![event]);
        let bytes = request.snapshot_bytes();
        let mut operation = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
        let control = operation.control();
        let operation_wakes = arm_capture_future(operation.as_mut());
        let mut capture = Box::pin(
            handle
                .begin_drain_with_capture(
                    128,
                    prefix.source.take().unwrap(),
                    vec![0xcc; 16].into_boxed_slice(),
                )
                .unwrap(),
        );
        let capture_wakes = arm_capture_future(capture.as_mut());
        assert_eq!(handle.observer().snapshot_bytes_in_use(), bytes);
        assert_eq!(handle.observer().reply_cells_in_use(), 3);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
        if during_capture {
            state.lock().unwrap().complete_on_flush = true;
            release_owner.wait();
            entered_receiver
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            assert_eq!(control.phase(), Phase::ObservationFinished);
            assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
            assert_eq!(state.lock().unwrap().issues.len(), 2);
            assert_eq!(state.lock().unwrap().release_calls, 0);
            let trace = trace.lock().unwrap();
            assert_eq!(trace.capture_calls, 1);
            assert_eq!(trace.capture_submission_rosters[0].len(), 2);
            assert!(
                trace.capture_submission_rosters[0]
                    .iter()
                    .all(|(_, status)| *status == BackendPollV1::Succeeded)
            );
            assert!(
                !trace
                    .calls
                    .iter()
                    .any(|(call, _)| *call == "release_allocation_v1")
            );
        }
        handle
            .observer
            .sender
            .send(RuntimeAsyncEngineCommandV1::Stop)
            .unwrap();
        let waker = Waker::from(capture_wakes.clone());
        assert!(
            capture
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        assert_eq!(capture_wakes.0.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
        if during_capture {
            // Stop is queued behind the synchronous callback, not a copy interruption.
            release_sender.send(()).unwrap();
        } else {
            release_owner.wait();
        }
        let result = await_capture_future(capture.as_mut(), &capture_wakes);
        let captured = if during_capture {
            PrefixCaptureOutcome::Success.check(result, 2)
        } else {
            assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
            None
        };
        let operation_result = await_capture_future(operation.as_mut(), &operation_wakes);
        let waiter_result = await_capture_future(waiter.as_mut(), &waiter_wakes);
        assert!(control.same_operation(&operation.control()));
        assert_eq!(waiter.event(), event);
        if during_capture {
            let result = operation_result.unwrap();
            let submission = result.submission.unwrap();
            assert_ne!(submission.id(), predecessor);
            assert_eq!(submission.stream(), prefix.streams[1]);
            assert_eq!(
                result.observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(waiter_result.unwrap(), RuntimeCompletionStatusV1::Succeeded);
            assert_eq!(control.phase(), Phase::ObservationFinished);
            assert_capture_shutdown(engine, PrefixCaptureOutcome::Success, 2);
        } else {
            assert!(matches!(
                operation_result,
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
            assert!(matches!(
                waiter_result,
                Err(RuntimeAsyncEventErrorV1::EngineStopped)
            ));
            assert_eq!(control.phase(), Phase::StoppedBeforeSubmission);
            assert_capture_shutdown(engine, PrefixCaptureOutcome::Terminal, 1);
        }
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        assert_eq!(handle.observer().reply_cells_in_use(), 2);
        for wakes in [&capture_wakes, &operation_wakes, &waiter_wakes] {
            assert_eq!(wakes.0.load(AtomicOrdering::SeqCst), 1);
        }
        drop((capture, operation, waiter));
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        assert_eq!(
            handle.observer().drain_capture_bytes_in_use(),
            if during_capture { 16 } else { 0 }
        );
        if let Some(captured) = captured {
            assert_eq!(captured.as_bytes(), &[0x5a; 16]);
            drop(captured);
        }
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        let state = state.lock().unwrap();
        let trace = trace.lock().unwrap();
        assert_eq!(trace.capture_calls, usize::from(during_capture));
        assert_eq!(state.issues.len(), if during_capture { 2 } else { 1 });
        assert_eq!(
            trace
                .calls
                .iter()
                .filter(|(call, _)| *call == "submit_v1")
                .count(),
            state.issues.len()
        );
        if during_capture {
            let mut released = trace.released_submissions.clone();
            released.sort_unstable();
            let mut expected: Vec<_> = state.issues.iter().map(|issue| issue.1).collect();
            expected.sort_unstable();
            assert_eq!(released, expected);
            assert_eq!(
                trace.capture_requests,
                vec![(1, prefix.native_source, 8, 16)]
            );
            assert!(state.statuses.is_empty());
        } else {
            assert!(trace.released_submissions.is_empty());
            assert!(trace.capture_requests.is_empty());
            assert_eq!(state.statuses[&state.issues[0].1], BackendPollV1::Pending);
            assert!(
                !trace.calls.iter().any(|(call, _)| matches!(
                    *call,
                    "release_allocation_v1" | "finalize" | "drop"
                ))
            );
        }
    }
}
