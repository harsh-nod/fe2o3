use super::*;

fn group_config(bytes: usize) -> RuntimeAsyncEngineConfigV1 {
    RuntimeAsyncEngineConfigV1::default()
        .with_drain_capture_group_byte_capacity(bytes)
        .unwrap()
}

fn register_ranges(
    context: &mut RuntimeContextV1<ThreadBoundBackend>,
    lengths: &[usize],
) -> Box<[RuntimeHostCaptureSourceV1]> {
    lengths
        .iter()
        .map(|&length| {
            let allocation = context
                .allocate(
                    context.devices()[0].id(),
                    RuntimeMemoryKindV1::HostVisible,
                    length as u64,
                    8,
                )
                .unwrap();
            context
                .prepare_host_drain_capture_v1(allocation, 0, length)
                .unwrap()
        })
        .collect()
}

fn group_sources(
    handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
    lengths: &'static [usize],
) -> Box<[RuntimeHostCaptureSourceV1]> {
    handle
        .observer()
        .try_with_context(move |context| register_ranges(context, lengths))
        .unwrap()
}

#[test]
fn group_capture_orders_uneven_ranges_and_retains_one_result_past_owner_shutdown() {
    let trace = Arc::new(Mutex::new(OwnerTrace {
        capture_fill_by_allocation: true,
        ..OwnerTrace::default()
    }));
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        trace.clone(),
        group_config(MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1),
    );
    let mut sources = group_sources(&handle, &[3, 7, 5]);
    sources.swap(0, 2);
    let report = join_command(
        handle
            .begin_drain_with_capture_group(64, sources, vec![0xcc; 15].into_boxed_slice())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.drain.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    let bytes = report.capture.unwrap();
    let trace = trace.lock().unwrap();
    assert_eq!(trace.capture_calls, 3);
    assert_eq!(
        trace
            .capture_requests
            .iter()
            .map(|r| r.3)
            .collect::<Vec<_>>(),
        [5, 7, 3]
    );
    let expected: Vec<_> = trace
        .capture_requests
        .iter()
        .flat_map(|r| vec![r.1 as u8; r.3])
        .collect();
    assert_eq!(bytes.as_bytes(), expected);
    assert!(expected.windows(2).any(|w| w[0] != w[1]));
    drop(trace);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 15);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    drop(bytes);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
}

#[test]
fn group_capture_revalidates_the_last_source_before_reading_the_first() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        trace.clone(),
        group_config(15),
    );
    let sources = group_sources(&handle, &[3, 7, 5]);
    let last = sources[2].allocation();
    let release = paused_owner(&handle);
    let removed = handle
        .observer()
        .enqueue_with_context(move |context| {
            context.release_allocation(last).unwrap();
        })
        .unwrap();
    let future = handle
        .begin_drain_with_capture_group(64, sources, vec![0; 15].into_boxed_slice())
        .unwrap();
    release.wait();
    join_command(removed).unwrap();
    let report = join_command(future).unwrap();
    assert!(matches!(
        report.capture,
        Err(CaptureError::UnknownAllocation)
    ));
    assert_eq!(trace.lock().unwrap().capture_calls, 0);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
}

#[test]
fn group_capture_second_read_failure_or_panic_discards_prefix_and_skips_suffix() {
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
            capture_fault_at: Some(2),
            ..OwnerTrace::default()
        }));
        let (engine, handle) = start_with_config(
            Arc::new(Mutex::new(MockState::default())),
            trace.clone(),
            group_config(15),
        );
        let sources = group_sources(&handle, &[3, 7, 5]);
        let result = join_command(
            handle
                .begin_drain_with_capture_group(64, sources, vec![0; 15].into_boxed_slice())
                .unwrap(),
        );
        match mode {
            0 => assert!(matches!(
                result.unwrap().capture,
                Err(CaptureError::NativeRejected)
            )),
            1 => assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::CaptureFailed(
                    CaptureError::NativeUncertain
                ))
            )),
            2 => assert!(matches!(
                result.unwrap().capture,
                Err(CaptureError::Pending)
            )),
            3 => assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            )),
            _ => unreachable!(),
        }
        let shutdown = engine.shutdown().unwrap();
        assert_eq!(
            shutdown.disposition,
            if mode == 1 || mode == 3 {
                RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
            } else {
                RuntimeAsyncOwnedDispositionV1::Released
            }
        );
        assert_eq!(trace.lock().unwrap().capture_calls, 2);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
    }
}

#[test]
fn group_capture_rejection_returns_exact_boxes_and_preserves_open_admission() {
    for mode in 0..8 {
        let capacity = if mode == 0 {
            0
        } else if mode == 3 {
            14
        } else {
            64
        };
        let config = group_config(capacity).with_reply_capacity(1).unwrap();
        let (engine, handle) = start_with_config(
            Arc::new(Mutex::new(MockState::default())),
            Arc::new(Mutex::new(OwnerTrace::default())),
            config,
        );
        let sources = match mode {
            4 => group_sources(&handle, &[]),
            5 => group_sources(&handle, &[1; MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_RANGES_V1 + 1]),
            _ => group_sources(&handle, &[3, 7, 5]),
        };
        let mut foreign = None;
        let sources = if mode == 6 {
            let (other, h) = start_with_config(
                Arc::new(Mutex::new(MockState::default())),
                Arc::new(Mutex::new(OwnerTrace::default())),
                config,
            );
            let mut ranges = sources;
            let mut imported = group_sources(&h, &[5]);
            std::mem::swap(&mut ranges[2], &mut imported[0]);
            foreign = Some(other);
            ranges
        } else {
            sources
        };
        let blocker = if mode == 7 {
            Some(handle.observer().enqueue_with_context(|_| ()).unwrap())
        } else {
            None
        };
        let source_ptr = sources.as_ptr();
        let destination = vec![0xcc; if mode == 2 { 14 } else { 15 }].into_boxed_slice();
        let destination_ptr = destination.as_ptr();
        let failure = handle
            .begin_drain_with_capture_group(if mode == 1 { 0 } else { 64 }, sources, destination)
            .err()
            .unwrap();
        let (error, returned_sources, returned_destination) = failure.into_parts();
        assert_eq!(
            error,
            match mode {
                0 => AdmissionError::Disabled,
                1 => AdmissionError::InvalidTickBudget,
                2 => AdmissionError::InvalidDestination,
                3 => AdmissionError::StorageCapacity,
                4 | 5 => AdmissionError::InvalidSources,
                6 => AdmissionError::ForeignContext,
                7 => AdmissionError::ReplyCapacity,
                _ => unreachable!(),
            }
        );
        assert_eq!(returned_sources.as_ptr(), source_ptr);
        assert_eq!(returned_destination.as_ptr(), destination_ptr);
        assert!(returned_destination.iter().all(|&byte| byte == 0xcc));
        drop(blocker);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        // Await the accepted prefix before shutdown, which otherwise interrupts drain.
        let report = join_command(handle.begin_drain(64).unwrap()).unwrap();
        assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
        assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
        if let Some(other) = foreign {
            assert!(other.shutdown().unwrap().cleanup.unwrap().is_complete());
        }
    }
}

#[test]
fn group_capture_current_thread_drop_before_drive_keeps_cutoff_and_completes_capture() {
    use current_thread_tests::{drive, start_current};
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (mut engine, handle) = start_current(
        trace.clone(),
        group_config(MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1),
    );
    let sources = drive(
        &mut engine,
        handle
            .observer()
            .enqueue_with_context(|context| register_ranges(context, &[3, 7, 5]))
            .unwrap(),
    )
    .unwrap();
    let future = handle
        .begin_drain_with_capture_group(64, sources, vec![0; 15].into_boxed_slice())
        .unwrap();
    drop(future);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 15);
    assert!(matches!(
        handle.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    for _ in 0..64 {
        if engine.tick().unwrap() == RuntimeAsyncTickV1::Stopped {
            break;
        }
    }
    let shutdown = engine.shutdown();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(trace.lock().unwrap().capture_calls, 3);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn group_capacity_is_accepted_by_both_transferable_owner_constructors() {
    for progress in [false, true] {
        let mut context = RuntimeContextV1::open(MockBackend {
            state: Arc::new(Mutex::new(MockState::default())),
        })
        .unwrap();
        let config = group_config(MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1);
        if progress {
            let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
                context,
                config,
                RuntimeAsyncProgressConfigV1::default(),
            )
            .unwrap();
            assert_eq!(
                join_command(handle.begin_drain(64).unwrap())
                    .unwrap()
                    .outcome,
                RuntimeAsyncDrainOutcomeV1::Quiescent
            );
            context = engine.into_context().unwrap();
        } else {
            let (engine, _handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
            context = engine.into_context().unwrap();
        }
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn group_capacity_does_not_raise_single_capture_limit_and_accepts_larger_group() {
    let length = MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_BYTES_V1 + 1;
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        trace.clone(),
        group_config(MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1),
    );
    let source = handle
        .observer()
        .try_with_context(move |context| {
            let sources = register_ranges(context, &[length]);
            sources.into_vec().pop().unwrap()
        })
        .unwrap();
    let destination = vec![0; length].into_boxed_slice();
    let address = destination.as_ptr();
    let failure = handle
        .begin_drain_with_capture(64, source, destination)
        .err()
        .unwrap();
    let (error, source, destination) = failure.into_parts();
    assert_eq!(error, AdmissionError::StorageCapacity);
    assert_eq!(destination.as_ptr(), address);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    let report = join_command(
        handle
            .begin_drain_with_capture_group(64, vec![source].into_boxed_slice(), destination)
            .unwrap(),
    )
    .unwrap();
    let result = report.capture.unwrap();
    assert_eq!(result.len(), length);
    assert!(result.as_bytes().iter().all(|&byte| byte == 0x5a));
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), length);
    drop(result);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    assert_eq!(trace.lock().unwrap().capture_calls, 1);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
}

#[test]
fn group_capture_closed_admission_refunds_reservation_and_returns_exact_roster() {
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
        group_config(15),
    );
    let sources = group_sources(&handle, &[3, 7, 5]);
    let source_ptr = sources.as_ptr();
    let destination = vec![0xcc; 15].into_boxed_slice();
    let destination_ptr = destination.as_ptr();
    let release = paused_owner(&handle);
    let drain = handle.begin_drain(64).unwrap();
    let failure = handle
        .begin_drain_with_capture_group(64, sources, destination)
        .err()
        .unwrap();
    let (error, sources, destination) = failure.into_parts();
    assert_eq!(error, AdmissionError::AdmissionClosed);
    assert_eq!(sources.as_ptr(), source_ptr);
    assert_eq!(destination.as_ptr(), destination_ptr);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    release.wait();
    join_command(drain).unwrap();
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}

#[test]
fn group_capture_allows_overlapping_read_only_ranges_without_reordering() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        trace.clone(),
        group_config(12),
    );
    let sources = handle
        .observer()
        .try_with_context(|context| {
            let allocation = context
                .allocate(
                    context.devices()[0].id(),
                    RuntimeMemoryKindV1::HostVisible,
                    16,
                    8,
                )
                .unwrap();
            [(4, 3), (0, 4), (2, 5)]
                .into_iter()
                .map(|(offset, length)| {
                    context
                        .prepare_host_drain_capture_v1(allocation, offset, length)
                        .unwrap()
                })
                .collect::<Box<[_]>>()
        })
        .unwrap();
    let report = join_command(
        handle
            .begin_drain_with_capture_group(64, sources, vec![0; 12].into_boxed_slice())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.capture.unwrap().as_bytes(), &[0x5a; 12]);
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .capture_requests
            .iter()
            .map(|r| (r.2, r.3))
            .collect::<Vec<_>>(),
        [(4, 3), (0, 4), (2, 5)]
    );
    assert!(
        trace
            .capture_requests
            .iter()
            .all(|r| r.1 == trace.capture_requests[0].1)
    );
    drop(trace);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
}

#[test]
fn group_capture_checked_extent_overflow_does_not_reserve_or_close() {
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
        group_config(16),
    );
    // The mock allocates only logical IDs, not backing for these huge extents.
    let sources = group_sources(&handle, &[usize::MAX, 1]);
    let address = sources.as_ptr();
    let failure = handle
        .begin_drain_with_capture_group(64, sources, vec![0; 1].into_boxed_slice())
        .err()
        .unwrap();
    let (error, sources, _) = failure.into_parts();
    assert_eq!(error, AdmissionError::InvalidDestination);
    assert_eq!(sources.as_ptr(), address);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    assert_eq!(handle.observer().try_with_context(|_| 7), Ok(7));
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
}

#[test]
fn group_capture_accepts_the_exact_roster_bound() {
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        trace.clone(),
        group_config(16),
    );
    let sources = group_sources(&handle, &[1; MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_RANGES_V1]);
    let report = join_command(
        handle
            .begin_drain_with_capture_group(64, sources, vec![0; 16].into_boxed_slice())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report.capture.unwrap().as_bytes(), &[0x5a; 16]);
    assert_eq!(trace.lock().unwrap().capture_calls, 16);
    assert!(engine.shutdown().unwrap().cleanup.unwrap().is_complete());
}
