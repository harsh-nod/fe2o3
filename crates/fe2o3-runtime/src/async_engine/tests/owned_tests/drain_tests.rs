use super::*;
use crate::{RuntimeAsyncCancelResultV1 as Cancel, RuntimeAsyncOperationPhaseV1 as Phase};

#[path = "drain_capture_tests.rs"]
mod capture_tests;

struct DrainArgs(crate::RuntimeAllocationIdV1);

impl RuntimeArgumentsV1 for DrainArgs {
    const SIGNATURE_V1: [u8; 32] = [0x68; 32];

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        vec![0; 8]
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        vec![RuntimeBindingV1 {
            region: crate::RuntimeMemoryRegionV1 {
                allocation: self.0,
                access: crate::RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: 64,
            },
            kernarg_byte_offset: 0,
        }]
    }
}

struct DrainFixture {
    stream: RuntimeStreamIdV1,
    kernel: Arc<crate::TypedRuntimeKernelV1<DrainArgs>>,
    arguments: DrainArgs,
}

impl DrainFixture {
    fn new(handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>) -> Self {
        handle
            .observer()
            .try_with_context(|context| {
                let device = context.devices()[0].id();
                context
                    .configure_allocation_admission_v1(device, 64, 1)
                    .unwrap();
                let stream = context.create_stream(device).unwrap();
                let module = context.load_module(device, &[1]).unwrap();
                let kernel = Arc::new(
                    context
                        .resolve_kernel::<DrainArgs>(module, "drain")
                        .unwrap(),
                );
                let allocation = context
                    .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
                    .unwrap();
                context.write_allocation(allocation, 0, &[0; 64]).unwrap();
                let usage = context
                    .allocation_admission_usage_v1(device)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    usage.used,
                    crate::RuntimeResourceVectorV1::ZERO
                        .with(crate::RuntimeResourceKindV1::RequestedAllocationBytes, 64)
                        .with(crate::RuntimeResourceKindV1::AllocationRecords, 1)
                );
                assert_eq!(usage.retained_records, 1);
                Self {
                    stream,
                    kernel,
                    arguments: DrainArgs(allocation),
                }
            })
            .unwrap()
    }

    fn request(&self) -> RuntimeAsyncLaunchRequestV1<DrainArgs> {
        RuntimeAsyncLaunchRequestV1::new(
            self.stream,
            self.kernel.clone(),
            &self.arguments,
            geometry(),
            vec![],
        )
        .unwrap()
    }
}

fn paused_owner(handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>) -> Arc<Barrier> {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let (a, b) = (entered.clone(), release.clone());
    drop(
        handle
            .observer()
            .enqueue_with_context(move |_| {
                a.wait();
                b.wait();
            })
            .unwrap(),
    );
    entered.wait();
    release
}

#[test]
fn drn3a_accepted_submit_rejection_resolves_once_without_native_custody_or_reissue() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let fixture = DrainFixture::new(&handle);
    let release = paused_owner(&handle);
    state
        .lock()
        .unwrap()
        .submit_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError(
            "drain-submit-rejected",
        )));
    let request = fixture.request();
    let bytes = request.snapshot_bytes();
    assert!(bytes > 0);
    let mut operation = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
    let control = operation.control();
    assert!(control.same_operation(&operation.control()));
    assert_eq!(control.phase(), Phase::Queued);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), bytes);
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    let drain = handle.begin_drain(32).unwrap();
    assert!(matches!(
        handle.enqueue_launch_tracked(fixture.request()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(handle.observer().snapshot_bytes_in_use(), bytes);
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    release.wait();
    let report = join_command(drain).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.total_submissions, 0);
    assert_eq!(report.operations_remaining, 0);
    assert!(report.queued_commands_exhausted);
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    let cleanup = shutdown.cleanup.unwrap();
    assert!(cleanup.is_complete());
    assert_eq!(cleanup.allocation_credit_records_v1(), 0);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    let Poll::Ready(Ok(result)) = operation
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    else {
        panic!("accepted rejection must deliver its sole operation reply");
    };
    assert!(result.submission.is_none());
    assert!(matches!(
        result.observation,
        Err(RuntimeErrorV1::BackendRejected(MockError(
            "drain-submit-rejected"
        )))
    ));
    assert_eq!(result.rejected_observations, 0);
    assert_eq!(result.last_rejected_observation, None);
    assert!(control.same_operation(&operation.control()));
    assert_eq!(control.phase(), Phase::ObservationFinished);
    assert_eq!(
        control.cancel_before_submission(),
        Cancel::NotCancellable(Phase::ObservationFinished)
    );
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    drop(operation);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let state = state.lock().unwrap();
    assert!(state.issues.is_empty());
    assert!(state.statuses.is_empty());
    assert!(state.submit_failures.is_empty());
    assert_eq!(state.release_calls, 0);
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "submit_v1")
            .count(),
        1
    );
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "release_allocation_v1")
            .count(),
        1
    );
    assert!(trace.polled_submissions.is_empty());
}

#[test]
fn drn3a_repeated_observation_rejection_exhausts_drain_without_republishing_or_freeing() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(8, 8, 1, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), config);
    let fixture = DrainFixture::new(&handle);
    let release = paused_owner(&handle);
    state.lock().unwrap().poll_failures.extend(
        (0..64).map(|_| RuntimeBackendFailureV1::Rejected(MockError("drain-observation-rejected"))),
    );
    let request = fixture.request();
    let bytes = request.snapshot_bytes();
    let mut operation = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
    let control = operation.control();
    let retained_control = control.clone();
    assert_eq!(handle.observer().snapshot_bytes_in_use(), bytes);
    let drain = handle.begin_drain(8).unwrap();
    release.wait();
    let report = join_command(drain).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::BudgetExhausted);
    assert_eq!(report.ticks, 8);
    assert_eq!(report.retained_submissions.total_submissions, 1);
    assert_eq!(report.retained_submissions.pending, 1);
    assert_eq!(report.retained_submissions.succeeded, 0);
    assert_eq!(report.retained_submissions.quiescent_without_result, 0);
    assert_eq!(report.operations_remaining, 1);
    assert!(report.queued_commands_exhausted);
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    let cleanup = shutdown.cleanup.unwrap();
    assert!(!cleanup.is_complete());
    assert!(cleanup.is_terminal());
    assert_eq!(cleanup.retained().allocations, 1);
    assert_eq!(cleanup.retained().submissions, 1);
    assert_eq!(cleanup.retained().modules, 1);
    assert_eq!(cleanup.retained().streams, 1);
    assert_eq!(cleanup.allocation_credit_records_v1(), 1);
    assert!(control.same_operation(&retained_control));
    assert!(control.same_operation(&operation.control()));
    assert_eq!(control.phase(), Phase::StoppedAfterSubmission);
    assert_eq!(
        control.cancel_before_submission(),
        Cancel::NotCancellable(Phase::StoppedAfterSubmission)
    );
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    assert!(matches!(
        operation
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    ));
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    drop(operation);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let state = state.lock().unwrap();
    assert_eq!(state.issues.len(), 1);
    let native = state.issues[0].1;
    assert_eq!(state.statuses.len(), 1);
    assert_eq!(state.statuses[&native], BackendPollV1::Pending);
    assert!(state.poll_calls >= 2);
    assert_eq!(state.poll_failures.len(), 64 - state.poll_calls);
    assert!(!state.poll_failures.is_empty());
    assert_eq!(state.release_calls, 0);
    let trace = trace.lock().unwrap();
    assert_eq!(trace.polled_submissions.len(), state.poll_calls);
    assert!(trace.polled_submissions.iter().all(|id| *id == native));
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "submit_v1")
            .count(),
        1
    );
    assert!(!trace.calls.iter().any(|(call, _)| matches!(
        *call,
        "release_allocation_v1"
            | "release_submission_v1"
            | "destroy_stream_v1"
            | "finalize"
            | "drop"
    )));
}

#[test]
fn drn3a_preissue_cancellation_preserves_accepted_sibling_and_credit_lifetimes() {
    let state = Arc::new(Mutex::new(MockState {
        complete_on_flush: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let fixture = DrainFixture::new(&handle);
    let release = paused_owner(&handle);
    let request = fixture.request();
    let bytes = request.snapshot_bytes();
    let mut cancelled = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
    let cancelled_control = cancelled.control();
    let mut sibling = Box::pin(handle.enqueue_launch_tracked(fixture.request()).unwrap());
    let sibling_control = sibling.control();
    assert!(!cancelled_control.same_operation(&sibling_control));
    let drain = handle.begin_drain(64).unwrap();
    assert_eq!(
        cancelled_control.cancel_before_submission(),
        Cancel::CancelledBeforeSubmission
    );
    assert_eq!(
        cancelled_control.cancel_before_submission(),
        Cancel::AlreadyCancelled
    );
    assert_eq!(sibling_control.phase(), Phase::Queued);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 2 * bytes);
    assert_eq!(handle.observer().reply_cells_in_use(), 3);
    release.wait();
    let report = join_command(drain).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.total_submissions, 1);
    assert_eq!(report.retained_submissions.succeeded, 1);
    assert_eq!(report.retained_submissions.pending, 0);
    assert_eq!(report.operations_remaining, 0);
    assert!(report.queued_commands_exhausted);
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    let cleanup = shutdown.cleanup.unwrap();
    assert!(cleanup.is_complete());
    assert_eq!(cleanup.allocation_credit_records_v1(), 0);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    assert!(matches!(
        cancelled
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(
            RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission
        ))
    ));
    let Poll::Ready(Ok(result)) = sibling
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    else {
        panic!("accepted sibling must retain its successful reply");
    };
    assert_eq!(
        result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(result.submission.is_some());
    assert_eq!(result.rejected_observations, 0);
    assert_eq!(cancelled_control.phase(), Phase::CancelledBeforeSubmission);
    assert_eq!(sibling_control.phase(), Phase::ObservationFinished);
    assert!(cancelled_control.same_operation(&cancelled.control()));
    assert!(sibling_control.same_operation(&sibling.control()));
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    drop(cancelled);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    drop(sibling);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let state = state.lock().unwrap();
    assert_eq!(state.issues.len(), 1);
    assert_eq!(state.release_calls, 1);
    assert!(state.statuses.is_empty());
    let native = state.issues[0].1;
    let trace = trace.lock().unwrap();
    assert!(!trace.polled_submissions.is_empty());
    assert!(trace.polled_submissions.iter().all(|id| *id == native));
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "submit_v1")
            .count(),
        1
    );
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "release_allocation_v1")
            .count(),
        1
    );
}

#[test]
fn r65_drain_closes_full_channel_without_discarding_accepted_commands() {
    let config = RuntimeAsyncEngineConfigV1::new(1, 8, 1, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
        config,
    );
    assert!(matches!(
        handle.begin_drain(0),
        Err(RuntimeAsyncDrainErrorV1::InvalidTickBudget)
    ));
    assert!(matches!(
        handle.begin_drain(MAX_RUNTIME_ASYNC_DRAIN_TICKS_V1 + 1),
        Err(RuntimeAsyncDrainErrorV1::InvalidTickBudget)
    ));
    let release = paused_owner(&handle);
    let accepted = handle.observer().enqueue_with_context(|_| 7).unwrap();
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    let drain = handle.begin_drain(16).unwrap();
    assert!(matches!(
        handle.clone().begin_drain(16),
        Err(RuntimeAsyncDrainErrorV1::AdmissionClosed)
    ));
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    release.wait();
    let report = join_command(drain).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert!(report.queued_commands_exhausted);
    assert_eq!(join_command(accepted), Ok(7));
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
}

#[test]
fn r65_drain_progresses_raw_predecessor_and_dependent_owned_operation() {
    let state = Arc::new(Mutex::new(MockState {
        complete_on_flush: true,
        ..MockState::default()
    }));
    let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
    let (stream, kernel) = launch_fixture(&handle);
    let k = kernel.clone();
    let (event, other) = handle
        .observer()
        .try_with_context(move |context| {
            let raw = context
                .launch(stream, &k, &EmptyArgs, geometry(), &[])
                .unwrap();
            let event = context.record_event(&raw).unwrap();
            let other = context.create_stream(context.devices()[0].id()).unwrap();
            (event, other)
        })
        .unwrap();
    let operation = handle
        .launch(other, kernel, EmptyArgs, geometry(), vec![event])
        .unwrap();
    let report = join_command(handle.begin_drain(128).unwrap()).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.succeeded, 2);
    assert_eq!(
        join_command(operation).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    assert_eq!(state.lock().unwrap().issues.len(), 2);
}

#[test]
fn r65_drain_settles_all_waiters_but_not_persistent_progress_registrations() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let config = RuntimeAsyncEngineConfigV1::new(64, 64, 1, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(
        state.clone(),
        Arc::new(Mutex::new(OwnerTrace::default())),
        config,
    );
    let (stream, kernel) = launch_fixture(&handle);
    let events = handle
        .observer()
        .try_with_context(move |context| {
            let raw = context
                .launch(stream, &kernel, &EmptyArgs, geometry(), &[])
                .unwrap();
            (0..12)
                .map(|_| context.record_event(&raw).unwrap())
                .collect::<Vec<_>>()
        })
        .unwrap();
    let waiters: Vec<_> = events
        .into_iter()
        .map(|event| handle.observer().event_future(event).unwrap())
        .collect();
    let registration = handle.register_stream(stream).unwrap();
    state.lock().unwrap().complete_on_flush = true;
    let report = join_command(handle.begin_drain(128).unwrap()).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    for waiter in waiters {
        assert_eq!(
            futures_executor::block_on(waiter).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
    }
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    drop(registration);
}

#[test]
fn r65_drain_budget_exhaustion_retains_unresolved_native_custody() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let (stream, kernel) = launch_fixture(&handle);
    drop(
        handle
            .launch(stream, kernel, EmptyArgs, geometry(), vec![])
            .unwrap(),
    );
    let report = join_command(handle.begin_drain(8).unwrap()).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::BudgetExhausted);
    assert_eq!(report.ticks, 8);
    assert_eq!(report.retained_submissions.pending, 1);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(state.lock().unwrap().release_calls, 0);
    assert!(
        !trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(call, _)| matches!(*call, "destroy_stream_v1" | "finalize" | "drop"))
    );
}

#[test]
fn r65_drain_observer_drop_keeps_progress_and_cleanup() {
    let state = Arc::new(Mutex::new(MockState {
        complete_on_flush: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let (stream, kernel) = launch_fixture(&handle);
    drop(
        handle
            .launch(stream, kernel, EmptyArgs, geometry(), vec![])
            .unwrap(),
    );
    drop(handle.begin_drain(128).unwrap());
    wait_until(|| {
        trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(call, _)| *call == "finalize")
    });
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(state.lock().unwrap().issues.len(), 1);
}

#[test]
fn r65_drain_admission_race_has_one_finite_accepted_prefix() {
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
    let release = paused_owner(&handle);
    let race = Arc::new(Barrier::new(17));
    let executed = Arc::new(AtomicUsize::new(0));
    let workers: Vec<_> = (0..16)
        .map(|_| {
            let (h, gate, executed) = (handle.clone(), race.clone(), executed.clone());
            thread::spawn(move || {
                gate.wait();
                match h.observer().enqueue_with_context(move |_| {
                    executed.fetch_add(1, AtomicOrdering::SeqCst);
                }) {
                    Ok(future) => {
                        drop(future);
                        true
                    }
                    Err(RuntimeAsyncEngineCallErrorV1::EngineStopped) => false,
                    Err(error) => panic!("unexpected admission: {error:?}"),
                }
            })
        })
        .collect();
    race.wait();
    let drain = handle.begin_drain(128).unwrap();
    let accepted = workers
        .into_iter()
        .map(|worker| usize::from(worker.join().unwrap()))
        .sum::<usize>();
    release.wait();
    assert_eq!(
        join_command(drain).unwrap().outcome,
        RuntimeAsyncDrainOutcomeV1::Quiescent
    );
    assert_eq!(executed.load(AtomicOrdering::SeqCst), accepted);
    engine.shutdown().unwrap();
}

#[test]
fn r65_rejected_payload_drops_outside_admission_lock() {
    struct Reenter(Arc<drain::AdmissionV1>);
    impl Drop for Reenter {
        fn drop(&mut self) {
            self.0.close();
        }
    }
    let (engine, handle) = start(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
    );
    let release = paused_owner(&handle);
    let drain = handle.begin_drain(32).unwrap();
    let payload = Reenter(handle.observer.admission.clone());
    assert!(matches!(
        handle
            .observer()
            .enqueue_with_context(move |_| drop(payload)),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    release.wait();
    assert_eq!(
        join_command(drain).unwrap().outcome,
        RuntimeAsyncDrainOutcomeV1::Quiescent
    );
    engine.shutdown().unwrap();
}

#[test]
fn r65_stop_before_drain_pickup_resolves_future_and_retains_context() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(Arc::new(Mutex::new(MockState::default())), trace.clone());
    let _ = launch_fixture(&handle);
    let release = paused_owner(&handle);
    let drain = handle.begin_drain(128).unwrap();
    handle
        .observer
        .sender
        .send(RuntimeAsyncEngineCommandV1::Stop)
        .unwrap();
    release.wait();
    assert!(matches!(
        join_command(drain),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert!(
        !trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(call, _)| *call == "finalize")
    );
}

#[test]
fn r65_quiescent_poll_on_final_tick_is_not_budget_exhaustion() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
    let (stream, kernel) = launch_fixture(&handle);
    handle
        .observer()
        .try_with_context(move |context| {
            context
                .launch(stream, &kernel, &EmptyArgs, geometry(), &[])
                .unwrap()
        })
        .unwrap();
    let release = paused_owner(&handle);
    state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Quiescent(MockError("quiescent")));
    let drain = handle.begin_drain(1).unwrap();
    release.wait();
    let report = join_command(drain).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.quiescent_without_result, 1);
    assert_eq!(report.retained_submissions.succeeded, 0);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn r65_completed_reply_cells_retain_credit_until_actual_disposal() {
    let config = RuntimeAsyncEngineConfigV1::default()
        .with_reply_capacity(1)
        .unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
        config,
    );
    let mut response = Box::pin(handle.observer().enqueue_with_context(|_| 17).unwrap());
    handle.observer().try_with_context(|_| ()).unwrap();
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    assert!(matches!(
        response
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Ok(17))
    ));
    assert!(matches!(
        handle.clone().observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    drop(response);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let retained = handle.observer().enqueue_with_context(|_| 19).unwrap();
    let report = join_command(handle.begin_drain(32).unwrap()).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    assert_eq!(join_command(retained), Ok(19));
    engine.shutdown().unwrap();
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn r65_abandoned_reply_keeps_credit_until_producer_disposal() {
    let config = RuntimeAsyncEngineConfigV1::default()
        .with_reply_capacity(1)
        .unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
        config,
    );
    let release = paused_owner(&handle);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    release.wait();
    handle.observer().try_with_context(|_| ()).unwrap();
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    engine.shutdown().unwrap();
}

#[test]
fn r65_reply_payload_is_dropped_before_its_credit() {
    struct Payload(Arc<reply_budget::ReplyBudgetV1>);
    impl Drop for Payload {
        fn drop(&mut self) {
            assert_eq!(self.0.used(), 1);
        }
    }
    let budget = reply_budget::ReplyBudgetV1::new(1);
    let (mut reply, future) = owned::Reply::budgeted_pair(&budget).unwrap();
    reply.complete(Ok(Payload(budget.clone())));
    drop(future);
    assert_eq!(budget.used(), 1);
    drop(reply);
    assert_eq!(budget.used(), 0);
}

#[test]
fn r65_reply_capacity_configuration_is_checked() {
    for capacity in [0, MAX_RUNTIME_ASYNC_REPLIES_V1 + 1] {
        assert_eq!(
            RuntimeAsyncEngineConfigV1::default().with_reply_capacity(capacity),
            Err(RuntimeAsyncEngineConfigErrorV1::ReplyCapacity)
        );
    }
    assert_eq!(
        RuntimeAsyncEngineConfigV1::default()
            .with_reply_capacity(1)
            .unwrap()
            .reply_capacity(),
        1
    );
}

#[test]
fn r65_panic_before_drain_pickup_and_during_native_drain_resolves_without_cleanup() {
    for panic_site in 0..3 {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (engine, handle) = start(state.clone(), trace.clone());
        let (stream, kernel) = launch_fixture(&handle);
        handle
            .observer()
            .try_with_context(move |context| {
                context
                    .launch(stream, &kernel, &EmptyArgs, geometry(), &[])
                    .unwrap()
            })
            .unwrap();
        let release = paused_owner(&handle);
        if panic_site == 1 {
            trace.lock().unwrap().flush_panics = true;
        } else if panic_site == 2 {
            trace.lock().unwrap().poll_panics = true;
        } else {
            drop(
                handle
                    .observer()
                    .enqueue_with_context(|_| panic!("before drain pickup"))
                    .unwrap(),
            );
        }
        let drain = handle.begin_drain(32).unwrap();
        release.wait();
        assert!(matches!(
            join_command(drain),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        assert_eq!(
            engine.shutdown().unwrap().disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(state.lock().unwrap().release_calls, 0);
        assert!(
            !trace
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|(call, _)| *call == "finalize")
        );
    }
}

#[test]
fn r65_initially_terminal_owner_never_reports_quiescent_drain() {
    for _ in 0..16 {
        let trace = Arc::new(Mutex::new(OwnerTrace {
            initially_terminal: true,
            ..OwnerTrace::default()
        }));
        let (engine, handle) = start(Arc::new(Mutex::new(MockState::default())), trace.clone());
        match handle.begin_drain(8) {
            Ok(future) => assert!(matches!(
                join_command(future),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            )),
            Err(error) => assert_eq!(error, RuntimeAsyncDrainErrorV1::AdmissionClosed),
        }
        assert_eq!(
            engine.shutdown().unwrap().disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert!(
            !trace
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|(call, _)| *call == "finalize")
        );
    }
}

#[test]
fn r65_rejected_queue_and_closed_gate_return_reply_credit() {
    let config = RuntimeAsyncEngineConfigV1::new(1, 8, 1, 1, Duration::from_millis(1))
        .unwrap()
        .with_reply_capacity(3)
        .unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
        config,
    );
    let release = paused_owner(&handle);
    let queued = handle.observer().enqueue_with_context(|_| ()).unwrap();
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    let drain = handle.begin_drain(32).unwrap();
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    release.wait();
    join_command(drain).unwrap();
    join_command(queued).unwrap();
    engine.shutdown().unwrap();
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn r65_terminal_retained_roster_needs_no_poll_budget_or_flush() {
    let state = Arc::new(Mutex::new(MockState {
        complete_on_flush: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(64, 64, 1, 1, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(state, trace.clone(), config);
    let (stream, kernel) = launch_fixture(&handle);
    handle
        .observer()
        .try_with_context(move |context| {
            let mut retained = (0..32)
                .map(|_| {
                    context
                        .launch(stream, &kernel, &EmptyArgs, geometry(), &[])
                        .unwrap()
                })
                .collect::<Vec<_>>();
            context.flush_stream(stream).unwrap();
            for submission in &mut retained {
                context.poll(submission).unwrap();
            }
        })
        .unwrap();
    let release = paused_owner(&handle);
    trace.lock().unwrap().flush_panics = true;
    let drain = handle.begin_drain(1).unwrap();
    release.wait();
    let report = join_command(drain).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.succeeded, 32);
    assert_eq!(report.ticks, 1);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn r65_executor_drains_2048_accepted_operations_with_full_reply_budget() {
    const OPERATIONS: usize = 2048;
    let state = Arc::new(Mutex::new(MockState {
        complete_on_flush: true,
        ..MockState::default()
    }));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(4096, 4096, 64, 32, Duration::from_millis(1))
        .unwrap()
        .with_reply_capacity(OPERATIONS + 1)
        .unwrap();
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), config);
    let (stream, kernel) = launch_fixture(&handle);
    let release = paused_owner(&handle);
    let replies: Vec<_> = (0..OPERATIONS)
        .map(|_| {
            handle
                .launch(stream, kernel.clone(), EmptyArgs, geometry(), vec![])
                .unwrap()
        })
        .collect();
    assert_eq!(handle.observer().reply_cells_in_use(), OPERATIONS + 1);
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    let drain = handle.begin_drain(1024).unwrap();
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    executor.block_on(async {
        let mut drain = Box::pin(drain);
        let mut watchdog = Box::pin(tokio::time::sleep(Duration::from_secs(10)));
        let mut release = Some(release);
        let report = std::future::poll_fn(|cx| {
            // Check the timer first so its wake cannot rescue a lost owner wake.
            assert!(
                watchdog.as_mut().poll(cx).is_pending(),
                "drain watchdog expired"
            );
            let result = drain.as_mut().poll(cx);
            if result.is_pending()
                && let Some(release) = release.take()
            {
                release.wait();
            }
            result
        })
        .await
        .unwrap();
        assert!(
            release.is_none(),
            "drain must first register while the owner is paused"
        );
        assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
        assert_eq!(report.retained_submissions.succeeded, OPERATIONS);
        assert_eq!(report.retained_submissions.pending, 0);
        for reply in replies {
            assert_eq!(
                reply.await.unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
        }
    });
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    assert_eq!(state.lock().unwrap().issues.len(), OPERATIONS);
    let trace = trace.lock().unwrap();
    let owner = trace.calls[0].1;
    assert_ne!(owner, thread::current().id());
    assert!(trace.calls.iter().all(|(_, thread)| *thread == owner));
}
