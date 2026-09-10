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
